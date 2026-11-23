//! Pruebas de la API del módulo con un `Sistema` falso: nada toca wg, systemctl ni nft reales.

use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use netctl_mod_wireguard::Rutas;
use netctl_mod_wireguard::api::{self, Estado};
use netctl_mod_wireguard::aplicar::Sistema;
use serde_json::{Value, json};
use tower::ServiceExt;

/// Registra cada orden (`programa args…`) y lo que recibe por la entrada estándar.
#[derive(Default)]
struct Doble {
    llamadas: Mutex<Vec<(String, Option<String>)>>,
    wg_caido: bool,
    dump: Mutex<String>,
}

impl Doble {
    fn ordenes(&self) -> Vec<String> {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .map(|(o, _)| o.clone())
            .collect()
    }
    fn entrada_de(&self, prefijo: &str) -> Option<String> {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find(|(o, _)| o.starts_with(prefijo))
            .and_then(|(_, e)| e.clone())
    }
    fn olvidar(&self) {
        self.llamadas.lock().unwrap().clear();
    }
}

impl Sistema for Doble {
    fn ejecutar(
        &self,
        programa: &str,
        args: &[&str],
        entrada: Option<&str>,
    ) -> Result<String, String> {
        let orden = format!("{programa} {}", args.join(" "));
        self.llamadas
            .lock()
            .unwrap()
            .push((orden.clone(), entrada.map(String::from)));
        if orden.starts_with("wg show") {
            if self.wg_caido {
                return Err("Unable to access interface: No such device".into());
            }
            return Ok(self.dump.lock().unwrap().clone());
        }
        if orden.starts_with("wg-quick strip") {
            return Ok("[Interface]\nPrivateKey = x\n".into());
        }
        Ok(String::new())
    }
    fn ahora(&self) -> u64 {
        1_000_000
    }
}

struct Entorno {
    _dir: tempfile::TempDir,
    rutas: Rutas,
    doble: Arc<Doble>,
    estado: Arc<Estado>,
}

fn entorno_con(doble: Doble) -> Entorno {
    let dir = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(dir.path());
    let doble = Arc::new(doble);
    let estado = Estado::nuevo(rutas.clone(), doble.clone()).unwrap();
    Entorno {
        _dir: dir,
        rutas,
        doble,
        estado,
    }
}

async fn pedir(
    e: &Entorno,
    metodo: &str,
    ruta: &str,
    cuerpo: Option<Value>,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(metodo)
        .uri(ruta)
        .header("x-netctl-usuario", "admin");
    let body = match cuerpo {
        Some(v) => {
            req = req.header("content-type", "application/json");
            Body::from(v.to_string())
        }
        None => Body::empty(),
    };
    let r = api::router(e.estado.clone())
        .oneshot(req.body(body).unwrap())
        .await
        .unwrap();
    let estado = r.status();
    let bytes = r.into_body().collect().await.unwrap().to_bytes();
    let v = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into()));
    (estado, v)
}

/// Servidor configurado con el asistente y dos clientes.
async fn configurado(e: &Entorno) -> u64 {
    let (st, v) = pedir(
        e,
        "POST",
        "/asistente",
        Some(json!({
            "endpoint": "vpn.ejemplo.com", "puerto": "51820", "red_vpn": "10.8.0.0/24",
            "iface_salida": "eth0", "dns": "1.1.1.1",
            "intranet": [{"nombre": "LAN", "cidr": "192.168.1.0/24"}],
            "cliente": {"nombre": "Portátil de Ana", "internet": false}
        })),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    let (st, v) = pedir(
        e,
        "POST",
        "/clientes",
        Some(json!({"revision": v["revision"], "nombre": "movil"})),
    )
    .await;
    assert_eq!(st, StatusCode::CREATED, "{v}");
    v["revision"].as_u64().unwrap()
}

#[tokio::test]
async fn wg_caido() {
    let e = entorno_con(Doble {
        wg_caido: true,
        ..Default::default()
    });
    configurado(&e).await;
    e.estado.sondear().await;
    let (st, v) = pedir(&e, "GET", "/clientes", None).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["aviso"], "La interfaz wg0 no está activa");
    let clientes = v["clientes"].as_array().unwrap();
    assert_eq!(clientes.len(), 2);
    assert!(clientes.iter().all(|c| c["estado"]["conectado"] == false));
}

#[tokio::test]
async fn vivo_marca_conectados() {
    let e = entorno_con(Doble::default());
    configurado(&e).await;
    let (_, v) = pedir(&e, "GET", "/clientes", None).await;
    let clave = v["clientes"][0]["clave_publica"]
        .as_str()
        .unwrap()
        .to_string();
    let ahora = 1_000_000u64;
    *e.doble.dump.lock().unwrap() = format!(
        "priv\tpub\t51820\toff\n{clave}\t(none)\t203.0.113.7:4500\t10.8.0.2/32\t{}\t100\t200\t25\n",
        ahora - 30
    );
    e.estado.sondear().await;
    let (_, v) = pedir(&e, "GET", "/clientes", None).await;
    assert!(v["aviso"].is_null(), "{v}");
    assert_eq!(v["clientes"][0]["estado"]["conectado"], true);
    assert_eq!(v["clientes"][0]["estado"]["endpoint"], "203.0.113.7:4500");
    assert_eq!(v["clientes"][1]["estado"]["ultimo_handshake"], Value::Null);
    assert_eq!(v["metricas"]["conectados"], 1);
    let (_, r) = pedir(&e, "GET", "/resumen", None).await;
    assert_eq!(r["metricas"][0]["valor"], "1");
}

#[tokio::test]
async fn editar_peer_usa_syncconf() {
    let e = entorno_con(Doble::default());
    let rev = configurado(&e).await;
    e.doble.olvidar();
    let (st, v) = pedir(
        &e,
        "PUT",
        "/clientes/1",
        Some(json!({"revision": rev, "internet": true})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let o = e.doble.ordenes();
    assert!(o.iter().any(|x| x == "wg syncconf wg0 /dev/stdin"), "{o:?}");
    assert!(o.iter().any(|x| x.starts_with("wg-quick strip ")), "{o:?}");
    assert!(!o.iter().any(|x| x.contains("restart")), "{o:?}");
    assert!(o.iter().any(|x| x == "nft -f -"), "{o:?}");
}

#[tokio::test]
async fn desactivar_quita_peer() {
    let e = entorno_con(Doble::default());
    let rev = configurado(&e).await;
    let (st, _) = pedir(
        &e,
        "POST",
        "/clientes/1/desactivar",
        Some(json!({"revision": rev})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let conf = std::fs::read_to_string(e.rutas.wireguard.join("wg0.conf")).unwrap();
    assert_eq!(conf.matches("[Peer]").count(), 1);
    assert!(!e.doble.entrada_de("nft").unwrap().contains("movil"));
}

#[tokio::test]
async fn cambiar_puerto_reinicia() {
    let e = entorno_con(Doble::default());
    let rev = configurado(&e).await;
    e.doble.olvidar();
    let (st, v) = pedir(
        &e,
        "PUT",
        "/servidor",
        Some(json!({"revision": rev, "puerto": "51999"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(v["reinicio"], true);
    let o = e.doble.ordenes();
    assert!(
        o.iter().any(|x| x == "systemctl restart wg-quick@wg0"),
        "{o:?}"
    );
    assert!(!o.iter().any(|x| x.starts_with("wg syncconf")), "{o:?}");
    // Cambiar solo el DNS no reinicia
    e.doble.olvidar();
    let (_, v) = pedir(
        &e,
        "PUT",
        "/servidor",
        Some(json!({"revision": v["revision"], "dns": "9.9.9.9"})),
    )
    .await;
    assert_eq!(v["reinicio"], false);
    assert!(!e.doble.ordenes().iter().any(|x| x.contains("restart")));
}

#[tokio::test]
async fn nat_segun_firewall() {
    let e = entorno_con(Doble::default());
    configurado(&e).await;
    assert!(e.doble.entrada_de("nft").unwrap().contains("masquerade"));
    let (st, _) = pedir(
        &e,
        "POST",
        "/evento",
        Some(json!({"tipo": "modulos_cambiados", "modulos": ["firewall", "wireguard"]})),
    )
    .await;
    assert!(st.is_success());
    assert!(!e.doble.entrada_de("nft").unwrap().contains("masquerade"));
    pedir(
        &e,
        "POST",
        "/evento",
        Some(json!({"tipo": "modulos_cambiados", "modulos": ["wireguard"]})),
    )
    .await;
    assert!(e.doble.entrada_de("nft").unwrap().contains("masquerade"));
}

#[tokio::test]
async fn firewall_instalado_al_arrancar() {
    let dir = tempfile::tempdir().unwrap();
    let rutas = Rutas::en(dir.path());
    std::fs::create_dir_all(rutas.modulos.join("firewall")).unwrap();
    std::fs::write(
        rutas.modulos.join("firewall/manifest.toml"),
        "id = \"firewall\"",
    )
    .unwrap();
    let doble = Arc::new(Doble::default());
    let estado = Estado::nuevo(rutas.clone(), doble.clone()).unwrap();
    let e = Entorno {
        _dir: dir,
        rutas,
        doble,
        estado,
    };
    configurado(&e).await;
    assert!(!e.doble.entrada_de("nft").unwrap().contains("masquerade"));
}

#[tokio::test]
async fn revision_vieja_409() {
    let e = entorno_con(Doble::default());
    let rev = configurado(&e).await;
    let (st, _) = pedir(
        &e,
        "PUT",
        "/clientes/0",
        Some(json!({"revision": rev, "extra": "10.9.0.0/24"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK);
    let (st, v) = pedir(
        &e,
        "PUT",
        "/clientes/0",
        Some(json!({"revision": rev, "extra": ""})),
    )
    .await;
    assert_eq!(st, StatusCode::CONFLICT, "{v}");
    assert!(v["error"].as_str().unwrap().contains("a la vez"));
}

#[tokio::test]
async fn validacion_422() {
    let e = entorno_con(Doble::default());
    let rev = configurado(&e).await;
    let (st, v) = pedir(
        &e,
        "PUT",
        "/clientes/0",
        Some(json!({"revision": rev, "ip": "10.8.0.3"})),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(
        v["errores"][0].as_str().unwrap().contains("repetida"),
        "{v}"
    );
}

#[tokio::test]
async fn conf_600() {
    let e = entorno_con(Doble::default());
    configurado(&e).await;
    let conf = e.rutas.wireguard.join("wg0.conf");
    let modo = std::fs::metadata(&conf).unwrap().permissions().mode() & 0o777;
    assert_eq!(modo, 0o600);
    assert!(!std::fs::read_to_string(&conf).unwrap().contains("PostUp"));
    let estado = e.rutas.datos.join("proyecto.json");
    assert_eq!(
        std::fs::metadata(&estado).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[tokio::test]
async fn descarga_conf_y_qr() {
    let e = entorno_con(Doble::default());
    configurado(&e).await;
    let req = Request::get("/clientes/0/conf")
        .body(Body::empty())
        .unwrap();
    let r = api::router(e.estado.clone()).oneshot(req).await.unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let disp = r.headers()["content-disposition"]
        .to_str()
        .unwrap()
        .to_string();
    assert_eq!(disp, "attachment; filename=\"Port_til_de_Ana.conf\"");
    let texto =
        String::from_utf8(r.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert!(texto.contains("Endpoint = vpn.ejemplo.com:51820"));
    let (st, svg) = pedir(&e, "GET", "/clientes/0/qr.svg", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(svg.as_str().unwrap().contains("<svg"));
}

#[tokio::test]
async fn sin_proyecto() {
    let e = entorno_con(Doble::default());
    let (st, v) = pedir(&e, "GET", "/clientes", None).await;
    assert_eq!(st, StatusCode::OK);
    assert_eq!(v["configurado"], false);
    let (st, v) = pedir(&e, "GET", "/resumen", None).await;
    assert_eq!(st, StatusCode::OK);
    assert!(v["metricas"].is_array());
    let (st, _) = pedir(&e, "PUT", "/servidor", Some(json!({"puerto": "1"}))).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn importar_json_antiguo() {
    let e = entorno_con(Doble::default());
    let json_antiguo = json!({
        "servidor": {"interfaz": "wg0", "endpoint": "vpn.casa.es", "puerto": 51820, "red_vpn": "10.8.0.0/24",
                     "ip_servidor": "10.8.0.1", "iface_salida": "eth0", "dns": "1.1.1.1", "nat": true, "firewall": true},
        "intranet": [{"id": 1, "nombre": "LAN", "cidr": "192.168.1.0/24"}],
        "clientes": [{"nombre": "tablet", "ip": "10.8.0.2", "keepalive": 25, "usar_psk": false}],
        "sig_id": 1
    });
    let (st, v) = pedir(&e, "POST", "/importar", Some(json_antiguo.clone())).await;
    assert_eq!(st, StatusCode::OK, "{v}");
    let (_, v) = pedir(&e, "GET", "/clientes", None).await;
    assert_eq!(v["clientes"][0]["nombre"], "tablet");
    assert_eq!(v["clientes"][0]["activo"], true);
    assert_eq!(v["clientes"][0]["keepalive"], "25");
    // Importar otra vez sin `reemplazar` no pisa lo que hay
    let (st, _) = pedir(&e, "POST", "/importar", Some(json_antiguo)).await;
    assert_eq!(st, StatusCode::CONFLICT);
}

#[tokio::test]
async fn adoptar_conf_con_postup() {
    let e = entorno_con(Doble::default());
    configurado(&e).await;
    // Simula un wg0.conf de wg-configurador con reglas iptables
    std::fs::write(
        e.rutas.wireguard.join("wg0.conf"),
        "[Interface]\nPostUp = iptables -A x\nPostDown = iptables -D x\n",
    )
    .unwrap();
    e.doble.olvidar();
    e.estado.arrancar().await;
    let o = e.doble.ordenes();
    let parar = o
        .iter()
        .position(|x| x == "systemctl stop wg-quick@wg0")
        .expect("debe parar antes de escribir");
    let reiniciar = o
        .iter()
        .position(|x| x == "systemctl restart wg-quick@wg0")
        .unwrap();
    assert!(parar < reiniciar);
    assert!(
        !std::fs::read_to_string(e.rutas.wireguard.join("wg0.conf"))
            .unwrap()
            .contains("PostUp")
    );
}

#[tokio::test]
async fn arrancar_sin_cambios_no_reinicia() {
    let e = entorno_con(Doble::default());
    configurado(&e).await;
    e.doble.olvidar();
    e.estado.arrancar().await;
    let o = e.doble.ordenes();
    assert!(
        !o.iter()
            .any(|x| x.contains("restart") || x.contains("syncconf")),
        "{o:?}"
    );
    assert!(
        o.iter().any(|x| x == "nft -f -"),
        "la tabla nft se recarga siempre al arrancar: {o:?}"
    );
}

#[tokio::test]
async fn interfaz_traversal_no_se_guarda_ni_borra() {
    let e = entorno_con(Doble::default());
    let rev = configurado(&e).await;
    // Fichero ajeno fuera de /etc/wireguard que el ataque intentaría borrar.
    let raiz = e.rutas.wireguard.parent().unwrap().parent().unwrap();
    let victima = raiz.join("root/x.conf");
    std::fs::create_dir_all(victima.parent().unwrap()).unwrap();
    std::fs::write(&victima, "no tocar").unwrap();
    let guardado = std::fs::read_to_string(e.rutas.datos.join("proyecto.json")).unwrap();

    let (st, v) = pedir(
        &e,
        "PUT",
        "/servidor",
        Some(json!({"revision": rev, "interfaz": "../../root/x"})),
    )
    .await;
    assert_eq!(st, StatusCode::UNPROCESSABLE_ENTITY, "{v}");
    assert!(v["errores"].as_array().is_some_and(|a| !a.is_empty()), "{v}");
    // Nada guardado: ni en disco ni en memoria.
    assert_eq!(
        std::fs::read_to_string(e.rutas.datos.join("proyecto.json")).unwrap(),
        guardado
    );
    let (_, s) = pedir(&e, "GET", "/servidor", None).await;
    assert_eq!(s["servidor"]["interfaz"], "wg0", "{s}");
    assert_eq!(s["revision"].as_u64(), Some(rev), "{s}");

    // Un cambio legítimo después no borra el fichero ajeno.
    let (st, v) = pedir(
        &e,
        "PUT",
        "/servidor",
        Some(json!({"revision": rev, "interfaz": "wg1"})),
    )
    .await;
    assert_eq!(st, StatusCode::OK, "{v}");
    assert_eq!(std::fs::read_to_string(&victima).unwrap(), "no tocar");

    // Defensa en profundidad: la ruta del .conf se niega para nombres no válidos.
    assert!(e.rutas.conf("../../root/x").is_err());
    assert!(e.rutas.conf("..").is_err());
    assert_eq!(e.rutas.conf("wg0").unwrap(), e.rutas.wireguard.join("wg0.conf"));
    netctl_mod_wireguard::aplicar::retirar(&*e.doble, &e.rutas, "../../root/x");
    assert!(victima.exists());
}
