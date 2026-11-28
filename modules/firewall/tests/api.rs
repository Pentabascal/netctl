//! Pruebas de la API del módulo Firewall con un `Sistema` falso (nada toca nft ni systemctl).

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use netctl_mod_firewall::api::{self, Estado};
use netctl_mod_firewall::sistema::{Falso, Rutas};
use serde_json::{Value, json};
use tower::ServiceExt;

const IP_ADDR: &str = r#"[{"ifname":"lo","operstate":"UNKNOWN","flags":["UP"],"address":"00:00:00:00:00:00","addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8}]},
{"ifname":"eth0","operstate":"UP","address":"52:54:00:00:00:01","addr_info":[{"family":"inet","local":"203.0.113.4","prefixlen":24}]},
{"ifname":"eth1","operstate":"UP","address":"52:54:00:00:00:02","addr_info":[{"family":"inet","local":"192.168.1.1","prefixlen":24}]},
{"ifname":"wg0","operstate":"UNKNOWN","flags":["UP"],"addr_info":[{"family":"inet","local":"10.8.0.1","prefixlen":24}]}]"#;

const NEIGH: &str = r#"[{"dst":"192.168.1.20","dev":"eth1","lladdr":"aa:bb:cc:dd:ee:01","state":["REACHABLE"]},
{"dst":"192.168.1.21","dev":"eth1","lladdr":"aa:bb:cc:dd:ee:02","state":["STALE"]}]"#;

struct Prueba {
    _dir: tempfile::TempDir,
    rutas: Rutas,
    s: Arc<Falso>,
    e: Arc<Estado>,
}

fn montar_con(plazo: Duration) -> Prueba {
    let dir = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(dir.path());
    std::fs::create_dir_all(&rutas.wireguard).unwrap();
    std::fs::write(rutas.wireguard.join("wg0.conf"), "[Interface]\nListenPort = 51820\n").unwrap();
    std::fs::create_dir_all(rutas.proc_route.parent().unwrap()).unwrap();
    std::fs::write(&rutas.proc_route, "Iface\tDestination\tGateway\neth0\t00000000\t0100710B\n").unwrap();
    let s = Arc::new(Falso::new());
    s.responder("ip -j addr", Ok(IP_ADDR));
    s.responder("ip -j neigh", Ok(NEIGH));
    s.responder("sshd -T", Ok("port 22\n"));
    s.responder("systemctl is-active", Err("inactive"));
    s.responder("ufw", Err("no existe"));
    *s.reloj.lock().unwrap() = 1_000_000;
    let e = Estado::nuevo(rutas.clone(), s.clone(), plazo).unwrap();
    Prueba { _dir: dir, rutas, s, e }
}

fn montar() -> Prueba {
    montar_con(Duration::from_secs(60))
}

async fn pedir(p: &Prueba, metodo: &str, ruta: &str, cuerpo: Option<Value>) -> (StatusCode, Value) {
    let mut r = Request::builder().method(metodo).uri(ruta).header("x-netctl-usuario", "admin");
    let body = match cuerpo {
        Some(c) => {
            r = r.header("content-type", "application/json");
            Body::from(c.to_string())
        }
        None => Body::empty(),
    };
    let resp = api::router(p.e.clone()).oneshot(r.body(body).unwrap()).await.unwrap();
    let st = resp.status();
    let b = resp.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&b).unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&b).into()));
    (st, v)
}

/// Asistente aplicado y confirmado: router eth0 Externa, eth1 Interna, wg0 VPN.
async fn configurado() -> Prueba {
    let p = montar();
    let (st, v) = pedir(&p, "POST", "/asistente", Some(json!({"interfaces": {"eth0": "externa", "eth1": "interna", "wg0": "vpn"}}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let id = v["pendiente"]["id"].as_u64().unwrap();
    let (st, _) = pedir(&p, "POST", &format!("/confirmar/{id}"), None).await;
    assert_eq!(st, StatusCode::OK);
    p
}

async fn revision(p: &Prueba) -> u64 {
    pedir(p, "GET", "/config", None).await.1["config"]["revision"].as_u64().unwrap()
}

#[tokio::test]
async fn sin_config_resumen_y_asistente() {
    let p = montar();
    let (st, v) = pedir(&p, "GET", "/resumen", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["metricas"][0]["valor"], "Sin configurar");
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    assert_eq!(v["configurado"], false);
    let (_, v) = pedir(&p, "GET", "/asistente", None).await;
    let ifs = v["interfaces"].as_array().unwrap();
    assert_eq!(ifs.len(), 3, "sin lo: {v}");
    let eth0 = ifs.iter().find(|i| i["nombre"] == "eth0").unwrap();
    assert_eq!(eth0["zona_sugerida"], "externa");
    assert_eq!(v["otro_firewall"], Value::Null);
}

#[tokio::test]
async fn asistente_aplica_y_traspasa() {
    let p = montar();
    p.s.responder("systemctl is-active firewalld", Ok("active\n"));
    let (_, v) = pedir(&p, "GET", "/asistente", None).await;
    assert_eq!(v["otro_firewall"], "firewalld");
    let (st, v) = pedir(&p, "POST", "/asistente", Some(json!({"interfaces": {"eth0": "externa", "eth1": "interna", "wg0": "vpn"}}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert_eq!(v["pendiente"]["segundos"], 60);
    assert!(p.s.ordenes().contains(&"systemctl disable --now firewalld".to_string()));
    assert!(!p.s.entradas("nft -f").is_empty());
    // interfaz que no existe → 422
    let (st, _) = pedir(&p, "POST", "/asistente", Some(json!({"interfaces": {"eth\"9": "externa"}}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn politica_devuelve_pendiente() {
    let p = configurado().await;
    let rev = revision(&p).await;
    let (st, v) = pedir(&p, "PUT", "/politicas/externa/gateway", Some(json!({"politica": "permitir", "revision": rev}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (_, v) = pedir(&p, "GET", "/pendiente", None).await;
    assert!(v["pendiente"]["id"].is_u64());
    let (_, v) = pedir(&p, "GET", "/politicas/externa/gateway", None).await;
    assert_eq!(v["politica"], "permitir");
    assert_eq!(v["de_serie"], "bloquear");
}

#[tokio::test]
async fn sin_confirmar_vuelve_atras() {
    let p = montar_con(Duration::from_millis(100));
    let (_, v) = pedir(&p, "POST", "/asistente", Some(json!({"interfaces": {"eth0": "externa", "eth1": "interna", "wg0": "vpn"}}))).await;
    let id = v["pendiente"]["id"].as_u64().unwrap();
    pedir(&p, "POST", &format!("/confirmar/{id}"), None).await;
    let rev = revision(&p).await;
    pedir(&p, "PUT", "/politicas/externa/gateway", Some(json!({"politica": "permitir", "revision": rev}))).await;
    tokio::time::sleep(Duration::from_millis(400)).await;
    let (_, v) = pedir(&p, "GET", "/politicas/externa/gateway", None).await;
    assert_eq!(v["politica"], "bloquear");
    let (_, v) = pedir(&p, "GET", "/pendiente", None).await;
    assert_eq!(v["pendiente"], Value::Null);
    let (st, v) = pedir(&p, "POST", "/confirmar/999", None).await;
    assert_eq!(st, StatusCode::NOT_FOUND);
    assert!(v["error"].as_str().unwrap().contains("se deshizo"));
}

#[tokio::test]
async fn revision_vieja_409() {
    let p = configurado().await;
    let rev = revision(&p).await;
    let (st, _) = pedir(&p, "PUT", "/politicas/externa/gateway", Some(json!({"politica": "permitir", "revision": rev - 1}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn reglas_crear_editar_ordenar_borrar() {
    let p = configurado().await;
    let regla = json!({"nombre": "Web", "origen_zona": "externa", "destino_zona": "gateway", "accion": "permitir",
        "protocolo": "tcp", "puertos": {"tipo": "lista", "valor": [{"desde": 80, "hasta": 80}]}});
    let (st, v) = pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let id1 = v["id"].as_u64().unwrap();
    let (_, v) = pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    let id2 = v["id"].as_u64().unwrap();
    let (st, _) = pedir(&p, "POST", "/reglas/orden", Some(json!({"ids": [id2, id1], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (_, v) = pedir(&p, "GET", "/reglas", None).await;
    assert_eq!(v["reglas"][0]["id"], id2);
    assert!(v["fijas"].as_array().unwrap().iter().any(|f| f["id"] == "fija_panel"));
    let mut editada = v["reglas"][0].clone();
    editada["nombre"] = json!("Web 2");
    let (st, _) = pedir(&p, "PUT", &format!("/reglas/{id2}"), Some(json!({"regla": editada, "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (st, _) = pedir(&p, "DELETE", &format!("/reglas/{id1}?revision={}", revision(&p).await), None).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (_, v) = pedir(&p, "GET", "/reglas", None).await;
    assert_eq!(v["reglas"].as_array().unwrap().len(), 1);
    assert_eq!(v["reglas"][0]["nombre"], "Web 2");
    // regla con error → 422 con el texto
    let mala = json!({"nombre": "x", "origen_zona": "externa", "destino_zona": "no_existe", "accion": "permitir"});
    let (st, v) = pedir(&p, "POST", "/reglas", Some(json!({"regla": mala, "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(v["errores"][0].as_str().unwrap().contains("zona"));
}

#[tokio::test]
async fn regla_fija_no_borrable() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "DELETE", "/reglas/fija_panel", None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    assert!(v["error"].as_str().unwrap().contains("acceso al panel"));
}

fn reenvio_json(puerto: u16) -> Value {
    json!({"nombre": "Escritorio de Ana", "equipo": {"tipo": "equipo", "mac": "aa:bb:cc:dd:ee:01", "ip": "192.168.1.20", "nombre": "PC Ana"},
        "protocolo": "tcp", "externo": [{"desde": puerto, "hasta": puerto}]})
}

#[tokio::test]
async fn reenvio_pide_confirmacion() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": reenvio_json(25565), "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert!(v["pendiente"]["id"].is_u64());
    assert_eq!(v["como_conectarse"][0], "203.0.113.4:25565");
    assert!(p.s.entradas("nft -f").last().unwrap().contains("dnat ip to 192.168.1.20"));
}

#[tokio::test]
async fn reenvio_activar_desactivar_duplicar() {
    let p = configurado().await;
    let (_, v) = pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": reenvio_json(25565), "revision": revision(&p).await}))).await;
    let id = v["id"].as_u64().unwrap();
    let (st, _) = pedir(&p, "POST", &format!("/reenvios/{id}/desactivar"), Some(json!({"revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (_, v) = pedir(&p, "GET", "/reenvios", None).await;
    assert_eq!(v["reenvios"][0]["activo"], false);
    assert_eq!(v["reenvios"][0]["estado"], "desactivada");
    let (st, v) = pedir(&p, "POST", &format!("/reenvios/{id}/duplicar"), Some(json!({"revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (_, v) = pedir(&p, "GET", "/reenvios", None).await;
    let l = v["reenvios"].as_array().unwrap();
    assert_eq!(l.len(), 2);
    assert!(l[1]["nombre"].as_str().unwrap().contains("copia"));
    assert_eq!(l[1]["activo"], false, "la copia sale desactivada para no chocar");
}

#[tokio::test]
async fn comprobar_avisa_rdp() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "POST", "/reenvios/comprobar", Some(json!({"reenvio": reenvio_json(3389)}))).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["avisos"][0].as_str().unwrap().contains("Escritorio remoto"), "{v}");
    let (_, v) = pedir(&p, "POST", "/reenvios/comprobar", Some(json!({"reenvio": reenvio_json(8443)}))).await;
    assert!(v["errores"][0].as_str().unwrap().contains("panel"));
    assert_eq!(v["siguiente_libre"], 8444);
}

#[tokio::test]
async fn comprobar_destino_vpn_explica() {
    let p = configurado().await;
    let mut r = reenvio_json(80);
    r["equipo"] = json!({"tipo": "ip", "red": "10.8.0.5/32"});
    let (_, v) = pedir(&p, "POST", "/reenvios/comprobar", Some(json!({"reenvio": r}))).await;
    assert!(v["avisos"].as_array().unwrap().iter().any(|a| a.as_str().unwrap().contains("VPN")), "{v}");
}

#[tokio::test]
async fn estado_de_reenvios() {
    let p = configurado().await;
    pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": reenvio_json(25565), "revision": revision(&p).await}))).await;
    let mut r = reenvio_json(8080);
    r["equipo"] = json!({"tipo": "equipo", "mac": "aa:bb:cc:dd:ee:02", "ip": "192.168.1.21", "nombre": "NAS"});
    pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": r, "revision": revision(&p).await}))).await;
    let id = pedir(&p, "GET", "/pendiente", None).await.1["pendiente"]["id"].as_u64().unwrap();
    pedir(&p, "POST", &format!("/confirmar/{id}"), None).await;
    p.s.abiertos.lock().unwrap().push("192.168.1.20:25565".into());
    p.e.revisar().await;
    let (_, v) = pedir(&p, "GET", "/reenvios", None).await;
    let l = v["reenvios"].as_array().unwrap();
    assert_eq!(l[0]["estado"], "funcionando", "{v}");
    assert_eq!(l[1]["estado"], "desconectado", "{v}");
    p.s.abiertos.lock().unwrap().clear();
    p.e.revisar().await;
    let (_, v) = pedir(&p, "GET", "/reenvios", None).await;
    assert_eq!(v["reenvios"][0]["estado"], "no_responde");
}

#[tokio::test]
async fn revisar_desactiva_caducados() {
    let p = configurado().await;
    let mut r = reenvio_json(25565);
    r["caduca"] = json!(1_000_100);
    pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": r, "revision": revision(&p).await}))).await;
    let id = pedir(&p, "GET", "/pendiente", None).await.1["pendiente"]["id"].as_u64().unwrap();
    pedir(&p, "POST", &format!("/confirmar/{id}"), None).await;
    *p.s.reloj.lock().unwrap() = 1_000_200;
    p.e.revisar().await;
    let (_, v) = pedir(&p, "GET", "/reenvios", None).await;
    assert_eq!(v["reenvios"][0]["activo"], false);
    let (_, v) = pedir(&p, "GET", "/registro", None).await;
    assert!(v["entradas"][0]["resumen"].as_str().unwrap().contains("caducó"), "{v}");
}

#[tokio::test]
async fn grupos_y_grupo_en_uso() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "POST", "/grupos", Some(json!({"tipo": "ip", "nombre": "Oficina", "miembros": [{"tipo": "ip", "red": "198.51.100.0/24"}], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let gid = v["id"].as_u64().unwrap();
    let regla = json!({"nombre": "Oficina entra", "origen_zona": "externa", "destino_zona": "gateway", "accion": "permitir", "origen": {"tipo": "grupo", "id": gid}});
    pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    let (st, v) = pedir(&p, "DELETE", &format!("/grupos/{gid}?revision={}", revision(&p).await), None).await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert!(v["usos"][0].as_str().unwrap().contains("Oficina entra"));
    let (_, v) = pedir(&p, "GET", "/grupos", None).await;
    assert_eq!(v["ip"][0]["usos"][0], "Regla «Oficina entra»");
    let (st, _) = pedir(&p, "POST", "/grupos", Some(json!({"tipo": "puertos", "nombre": "Web", "puertos": [{"desde": 80, "hasta": 80}], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn evento_desinstalando_restaura() {
    let p = montar();
    p.s.responder("systemctl is-active firewalld", Ok("active\n"));
    let (_, v) = pedir(&p, "POST", "/asistente", Some(json!({"interfaces": {"eth0": "externa", "eth1": "interna", "wg0": "vpn"}}))).await;
    let id = v["pendiente"]["id"].as_u64().unwrap();
    pedir(&p, "POST", &format!("/confirmar/{id}"), None).await;
    p.s.limpiar();
    let (st, _) = pedir(&p, "POST", "/evento", Some(json!({"tipo": "desinstalando", "restaurar": true}))).await;
    assert_eq!(st, StatusCode::OK);
    let o = p.s.ordenes();
    assert!(o.contains(&"nft delete table inet netctl".to_string()));
    assert!(o.contains(&"systemctl enable --now firewalld".to_string()), "{o:?}");
    let _ = &p.rutas;
}

#[tokio::test]
async fn evento_desinstalando_sin_restaurar_solo_quita() {
    let p = configurado().await;
    p.s.limpiar();
    pedir(&p, "POST", "/evento", Some(json!({"tipo": "desinstalando", "restaurar": false}))).await;
    assert_eq!(p.s.ordenes(), vec!["nft delete table inet netctl".to_string()]);
}

#[tokio::test]
async fn equipos_lista_vecinos() {
    let p = montar();
    let (st, v) = pedir(&p, "GET", "/equipos", None).await;
    assert_eq!(st, StatusCode::OK);
    let l = v.as_array().unwrap();
    assert_eq!(l.len(), 2);
    assert_eq!(l[0]["mac"], "aa:bb:cc:dd:ee:01");
    assert_eq!(l[0]["conectado"], true);
    assert_eq!(l[0]["red"], "eth1");
}

#[tokio::test]
async fn contadores_parsea_json() {
    let p = configurado().await;
    p.s.responder("nft -j list table inet netctl", Ok(r#"{"nftables":[{"metainfo":{}},
{"rule":{"family":"inet","table":"netctl","chain":"prerouting","comment":"f7_Minecraft","expr":[{"match":{}},{"counter":{"packets":12,"bytes":3400}},{"dnat":{}}]}},
{"rule":{"family":"inet","table":"netctl","chain":"prerouting","comment":"f7_Minecraft","expr":[{"counter":{"packets":3,"bytes":100}}]}},
{"rule":{"family":"inet","table":"netctl","chain":"input","comment":"p_externa_gateway","expr":[{"counter":{"packets":99,"bytes":5000}}]}}]}"#));
    let (_, v) = pedir(&p, "GET", "/contadores", None).await;
    assert_eq!(v["f7"]["paquetes"], 15);
    assert_eq!(v["f7"]["bytes"], 3500);
    assert_eq!(v["p_externa_gateway"]["paquetes"], 99);
}

#[tokio::test]
async fn generada_y_registro() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "GET", "/generada", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["texto"].as_str().unwrap().starts_with("table inet netctl {}"));
    let (_, v) = pedir(&p, "GET", "/registro", None).await;
    let l = v["entradas"].as_array().unwrap();
    assert_eq!(l[0]["estado"], "confirmado");
}

#[tokio::test]
async fn ajustes_ping_y_nat() {
    let p = configurado().await;
    let (st, _) = pedir(&p, "PUT", "/ajustes", Some(json!({"ping_externo": false, "nat": {"interna": false}, "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    assert_eq!(v["config"]["ping_externo"], false);
    assert_eq!(v["config"]["nat"]["interna"], false);
}

#[tokio::test]
async fn interfaces_cambiar_zona() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "PUT", "/interfaces/eth1", Some(json!({"zona": "dmz", "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (_, v) = pedir(&p, "GET", "/interfaces", None).await;
    let eth1 = v["interfaces"].as_array().unwrap().iter().find(|i| i["nombre"] == "eth1").unwrap().clone();
    assert_eq!(eth1["zona"], "dmz");
}

#[tokio::test]
async fn arranque_carga_confirmada() {
    let p = configurado().await;
    p.s.limpiar();
    let e2 = Estado::nuevo(p.rutas.clone(), p.s.clone(), Duration::from_secs(60)).unwrap();
    e2.arrancar().await;
    assert_eq!(p.s.entradas("nft -f").len(), 1);
}

#[tokio::test]
async fn config_trae_matriz() {
    let p = configurado().await;
    let regla = json!({"nombre": "Web", "origen_zona": "externa", "destino_zona": "gateway", "accion": "permitir"});
    pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    pedir(&p, "PUT", "/politicas/interna/externa", Some(json!({"politica": "bloquear", "revision": revision(&p).await}))).await;
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    let m = v["matriz"].as_array().unwrap();
    assert_eq!(m.len(), 6 * 6 - 1, "todas las parejas menos gateway→gateway");
    let c = |o: &str, d: &str| m.iter().find(|x| x["origen"] == o && x["destino"] == d).unwrap().clone();
    assert_eq!(c("externa", "gateway")["politica"], "bloquear");
    assert_eq!(c("externa", "gateway")["reglas"], 1);
    assert_eq!(c("interna", "externa")["politica"], "bloquear");
    assert_eq!(c("interna", "externa")["de_serie"], "permitir");
    assert_eq!(c("interna", "externa")["cambiada"], true);
}

#[tokio::test]
async fn equipos_usan_nombres_conocidos() {
    let p = configurado().await;
    pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": reenvio_json(25565), "revision": revision(&p).await}))).await;
    let (_, v) = pedir(&p, "GET", "/equipos", None).await;
    let l = v.as_array().unwrap();
    assert_eq!(l.iter().find(|e| e["mac"] == "aa:bb:cc:dd:ee:01").unwrap()["nombre"], "PC Ana");
    assert_eq!(l.iter().find(|e| e["mac"] == "aa:bb:cc:dd:ee:02").unwrap()["nombre"], "");
}

// ---- fase 2.5: página Zonas al estilo UniFi

#[tokio::test]
async fn politicas_agrupa_las_de_serie() {
    let p = configurado().await;
    let regla = json!({"nombre": "Web", "origen_zona": "externa", "destino_zona": "gateway", "accion": "permitir", "protocolo": "tcp",
        "puertos": {"tipo": "lista", "valor": [{"desde": 80, "hasta": 80}]}});
    pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    let (st, v) = pedir(&p, "GET", "/politicas", None).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let grupos = v["de_serie"].as_array().unwrap();
    assert_eq!(grupos.len(), 2, "Permitir todo y Bloquear todo: {v}");
    let total: usize = grupos.iter().map(|g| g["parejas"].as_array().unwrap().len()).sum();
    assert_eq!(total, 6 * 6 - 1);
    assert_eq!(grupos[0]["nombre"], "Permitir todo");
    let propia = &v["propias"][0];
    assert_eq!(propia["nombre"], "Web");
    assert_eq!(propia["zona_origen_nombre"], "Externa (internet)");
    assert_eq!(propia["zona_destino_nombre"], "Esta máquina");
    assert_eq!(propia["texto_origen"], "Cualquiera");
    assert_eq!(propia["texto_puertos"], "80");
    assert_eq!(propia["texto_puertos_origen"], "Cualquiera");
    // la matriz trae las reglas propias por pareja y los avisos
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    assert!(v["avisos"].as_array().unwrap().is_empty());
    assert_eq!(v["tarjetas_de_zona"]["interna"], json!(["eth1"]));
}

#[tokio::test]
async fn duplicar_regla() {
    let p = configurado().await;
    let regla = json!({"nombre": "Web", "origen_zona": "externa", "destino_zona": "gateway", "accion": "permitir"});
    let (_, v) = pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    let id = v["id"].as_u64().unwrap();
    let (st, v) = pedir(&p, "POST", &format!("/reglas/{id}/duplicar"), Some(json!({"revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (_, v) = pedir(&p, "GET", "/reglas", None).await;
    let r = v["reglas"].as_array().unwrap();
    assert_eq!(r.len(), 2);
    assert_eq!(r[1]["nombre"], "Web (copia)");
    assert_eq!(r[1]["activa"], false);
    assert_ne!(r[1]["id"], r[0]["id"]);
}

#[tokio::test]
async fn crear_zona_con_tarjetas() {
    let p = configurado().await;
    let (st, v) = pedir(&p, "POST", "/zonas", Some(json!({"nombre": "Cámaras", "tarjetas": ["eth1"], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let id = v["id"].as_str().unwrap().to_string();
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    assert_eq!(v["config"]["interfaces"]["eth1"], json!(id));
    let z = v["config"]["zonas"].as_array().unwrap().iter().find(|z| z["id"] == json!(id)).unwrap().clone();
    assert_eq!(z["tipo"], "personalizada");
    assert_eq!(z["nombre"], "Cámaras");
    // renombrar y quitar la tarjeta (vuelve a… ninguna zona: la tarjeta queda sin zona)
    let (st, v) = pedir(&p, "PUT", &format!("/zonas/{id}"), Some(json!({"nombre": "CCTV", "tarjetas": [], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    assert!(v["config"]["interfaces"].get("eth1").is_none());
    // nombre vacío
    let (st, _) = pedir(&p, "POST", "/zonas", Some(json!({"nombre": " ", "tarjetas": [], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn zona_de_serie_no_se_borra() {
    let p = configurado().await;
    let (st, _) = pedir(&p, "DELETE", &format!("/zonas/dmz?revision={}", revision(&p).await), None).await;
    assert_eq!(st, StatusCode::FORBIDDEN);
    // en las de serie solo cambian las tarjetas, no el nombre
    let (st, _) = pedir(&p, "PUT", "/zonas/dmz", Some(json!({"nombre": "Otra", "tarjetas": ["eth1"], "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    let z = v["config"]["zonas"].as_array().unwrap().iter().find(|z| z["id"] == "dmz").unwrap().clone();
    assert_eq!(z["nombre"], "DMZ");
    assert_eq!(v["config"]["interfaces"]["eth1"], "dmz");
}

#[tokio::test]
async fn zona_en_uso_409() {
    let p = configurado().await;
    let (_, v) = pedir(&p, "POST", "/zonas", Some(json!({"nombre": "Cámaras", "tarjetas": [], "revision": revision(&p).await}))).await;
    let id = v["id"].as_str().unwrap().to_string();
    let regla = json!({"nombre": "Cámaras fuera", "origen_zona": id, "destino_zona": "externa", "accion": "bloquear"});
    let (st, v) = pedir(&p, "POST", "/reglas", Some(json!({"regla": regla, "revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    let (st, v) = pedir(&p, "DELETE", &format!("/zonas/{id}?revision={}", revision(&p).await), None).await;
    assert_eq!(st, StatusCode::CONFLICT);
    assert_eq!(v["usos"], json!(["Cámaras fuera"]));
}

#[tokio::test]
async fn crear_nat_202() {
    let p = configurado().await;
    let rev = revision(&p).await;
    let nat = json!({ "nombre": "Web interna", "tipo": { "tipo": "dnat", "ip": "192.168.1.10", "puerto": 8080 },
        "iface": "eth0", "protocolo": "tcp", "puertos": { "tipo": "lista", "valor": [{ "desde": 80, "hasta": 80 }] } });
    let (st, v) = pedir(&p, "POST", "/nat", Some(json!({ "nat": nat, "revision": rev }))).await;
    assert_eq!(st, StatusCode::ACCEPTED, "{v}");
    assert!(v["pendiente"]["id"].is_u64());
    let id = v["id"].as_u64().unwrap();
    let (_, l) = pedir(&p, "GET", "/nat", None).await;
    assert_eq!(l["nat"][0]["id"], id);
    let rev = revision(&p).await;
    let (st, _) = pedir(&p, "POST", &format!("/nat/{id}/desactivar"), Some(json!({ "revision": rev }))).await;
    assert_eq!(st, StatusCode::ACCEPTED);
    let (_, l) = pedir(&p, "GET", "/nat", None).await;
    assert_eq!(l["nat"][0]["activa"], false);
    // Un DNAT con puerto sin protocolo se rechaza con el texto del plan.
    let rev = revision(&p).await;
    let malo = json!({ "nombre": "x", "tipo": { "tipo": "dnat", "ip": "192.168.1.10", "puerto": 8080 } });
    let (st, v) = pedir(&p, "POST", "/nat", Some(json!({ "nat": malo, "revision": rev }))).await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(v.to_string().contains("Para DNAT con puerto elige TCP o UDP."), "{v}");
    let rev = revision(&p).await;
    let (st, _) = pedir(&p, "DELETE", &format!("/nat/{id}?revision={rev}"), None).await;
    assert_eq!(st, StatusCode::ACCEPTED);
}

#[tokio::test]
async fn reanudar_con_choque_409() {
    let p = configurado().await;
    let (_, v) = pedir(&p, "POST", "/reenvios", Some(json!({"reenvio": reenvio_json(25565), "revision": revision(&p).await}))).await;
    let id = v["id"].as_u64().unwrap();
    let (_, v) = pedir(&p, "POST", &format!("/reenvios/{id}/duplicar"), Some(json!({"revision": revision(&p).await}))).await;
    let copia = v["id"].as_u64().unwrap();
    let (st, v) = pedir(&p, "POST", &format!("/reenvios/{copia}/activar"), Some(json!({"revision": revision(&p).await}))).await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert_eq!(v["error"], "No se puede reanudar: choca con «Escritorio de Ana».");
    assert_eq!(v["con"], id);
}

#[tokio::test]
async fn config_ips_externas_con_iface() {
    let p = configurado().await;
    let (_, v) = pedir(&p, "GET", "/config", None).await;
    assert_eq!(v["ips_externas"], json!([{"iface": "eth0", "ip": "203.0.113.4", "dinamica": false}]));
}
