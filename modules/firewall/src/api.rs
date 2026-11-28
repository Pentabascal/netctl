//! API HTTP del módulo (detrás del proxy del núcleo en `/api/m/firewall/…`).
//!
//! Los cambios llevan `revision`; si no coincide con la guardada, otra pestaña cambió algo
//! entre medias → 409. Todo cambio se aplica con vuelta atrás a los 60 s: la respuesta es
//! `202 {pendiente: {id, segundos}, revision}` y hay que llamar a `POST /confirmar/{id}`.
//! Errores de validación: 422 con `errores: [..]`.

use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::aplicar::{self, Aplicador, EnEspera};
use crate::leer::{self, Vecino};
use crate::modelo::*;
use crate::nft;
use crate::plantillas;
use crate::registro::{Entrada, EstadoCambio};
use crate::sistema::{Rutas, Sistema};
use crate::validar::{self, ip_reenvio};

const CONFLICTO: &str = "Alguien ha cambiado la configuración a la vez. Recarga y vuelve a intentarlo.";
const SIN_CONFIG: &str = "El firewall todavía no está configurado: usa el asistente.";
const NO_EXISTE: &str = "Eso ya no existe. Recarga la página.";
const FIJA: &str = "Esta regla protege tu acceso al panel, a SSH o a la VPN y no se puede borrar ni cambiar.";
const NO_CONFIRMABLE: &str =
    "Ya no se puede confirmar: el cambio se deshizo solo o ya estaba confirmado. Revisa la configuración actual.";

/// Lo que se va viendo de la red (no se guarda).
#[derive(Default)]
struct Vivo {
    vecinos: Vec<Vecino>,
    /// id de reenvío → «funcionando» / «no_responde» / «desconectado»
    estados: HashMap<u64, &'static str>,
    /// comentario corto → (paquetes, bytes)
    contadores: BTreeMap<String, (u64, u64)>,
    /// comentario corto → instante en que el contador subió por última vez
    ultima: HashMap<String, u64>,
}

/// Descarga de la base de países: si está en marcha, último intento y su error.
#[derive(Default)]
struct EstadoGeo {
    descargando: bool,
    intento: u64,
    error: Option<String>,
}

pub struct Estado {
    geo: Arc<std::sync::Mutex<EstadoGeo>>,
    /// IPs vistas de los destinos por dominio.
    dominios: Arc<std::sync::Mutex<crate::dominios::Cache>>,
    pub rutas: Rutas,
    sistema: Arc<dyn Sistema>,
    ap: Aplicador,
    cerrojo: tokio::sync::Mutex<()>,
    vivo: std::sync::Mutex<Vivo>,
    ultimas_fijas: std::sync::Mutex<Option<Fijas>>,
}

pub struct Fallo(StatusCode, Value);

impl Fallo {
    fn error(st: StatusCode, msg: &str) -> Self {
        Fallo(st, json!({ "error": msg }))
    }
    fn errores(v: Vec<String>) -> Self {
        Fallo(StatusCode::UNPROCESSABLE_ENTITY, json!({ "errores": v }))
    }
}

impl IntoResponse for Fallo {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

type R = Result<Response, Fallo>;

impl Estado {
    pub fn nuevo(rutas: Rutas, sistema: Arc<dyn Sistema>, plazo: Duration) -> anyhow::Result<Arc<Self>> {
        let ap = Aplicador::new(sistema.clone(), rutas.clone(), plazo);
        Ok(Arc::new(Self {
            rutas,
            sistema,
            ap,
            cerrojo: tokio::sync::Mutex::new(()),
            vivo: Default::default(),
            ultimas_fijas: Default::default(),
            dominios: Default::default(),
            geo: Default::default(),
        }))
    }

    /// Al arrancar el servicio: carga la última configuración confirmada.
    pub async fn arrancar(&self) {
        let _g = self.cerrojo.lock().await;
        let (s, r) = (self.sistema.clone(), self.rutas.clone());
        match self.ap.arrancar(|c| leer::fijas(&*s, &r, c)) {
            Ok(Some(c)) => *self.ultimas_fijas.lock().unwrap() = Some(leer::fijas(&*s, &r, &c)),
            Ok(None) => {}
            Err(e) => eprintln!("No se pudo cargar el firewall al arrancar: {e}"),
        }
    }

    /// Bucle de revisión cada 30 s (lo lanza `main`).
    pub async fn revisar_siempre(self: Arc<Self>) {
        let mut t = tokio::time::interval(Duration::from_secs(30));
        loop {
            t.tick().await;
            self.revisar().await;
        }
    }

    /// Una pasada: caducidades, IPs de equipos vinculados por MAC, reglas fijas, contadores y
    /// estado de los reenvíos.
    pub async fn revisar(&self) {
        let s = self.sistema.clone();
        let vecinos = leer::vecinos(&*s);
        self.vivo.lock().unwrap().vecinos = vecinos.clone();
        {
            let _g = self.cerrojo.lock().await;
            if self.ap.en_espera().is_none()
                && let Ok(Some(c)) = self.ap.almacen.cargar_confirmada()
            {
                let mut nueva = c.clone();
                let ahora = s.ahora();
                let mut msgs: Vec<String> = aplicar::revisar_caducados(&mut nueva, ahora)
                    .into_iter()
                    .map(|n| format!("El reenvío «{n}» caducó y se ha desactivado (no se ha borrado)."))
                    .collect();
                msgs.extend(aplicar::actualizar_ips(&mut nueva, &vecinos));
                let ifaces = leer::interfaces(&*s);
                if !ifaces.is_empty() {
                    nueva.direcciones = ifaces.iter().filter(|i| i.nombre != "lo").map(|i| (i.nombre.clone(), i.ips.clone())).collect();
                }
                let fijas = leer::fijas(&*s, &self.rutas, &nueva);
                let fijas_cambian = self.ultimas_fijas.lock().unwrap().as_ref() != Some(&fijas);
                if !msgs.is_empty() || fijas_cambian || nueva != c {
                    if nueva != c {
                        nueva.revision += 1;
                    }
                    let resumen = if msgs.is_empty() {
                        "Reglas fijas actualizadas (panel, SSH, WireGuard o IPs de la máquina)".to_string()
                    } else {
                        msgs.join(" ")
                    };
                    match self.ap.aplicar(&nueva, &fijas, "netctl", &resumen, false) {
                        Ok(_) => *self.ultimas_fijas.lock().unwrap() = Some(fijas),
                        Err(e) => eprintln!("Revisión periódica: {e}"),
                    }
                }
            }
        }
        if let Ok(Some(c)) = self.actual() {
            self.geo_si_toca(&c);
        }
        // Destinos por dominio: preguntar al DNS cada 5 min y volver a llenar los conjuntos (una
        // recarga de la tabla, o el «deshacer» de los 60 s, los deja vacíos).
        if let Ok(Some(c)) = self.actual() {
            let (s2, yo) = (s.clone(), self.dominios.clone());
            let _ = tokio::task::spawn_blocking(move || {
                let mut cache = yo.lock().unwrap();
                crate::dominios::refrescar(&c, &*s2, &mut cache);
            })
            .await;
        }
        // Contadores
        if let Ok(j) = s.ejecutar("nft", &["-j", "list", "table", "inet", "netctl"], None) {
            let nuevos = leer::parse_contadores(&j);
            let ahora = s.ahora();
            let mut v = self.vivo.lock().unwrap();
            for (k, (p, _)) in &nuevos {
                if v.contadores.get(k).is_some_and(|(a, _)| p > a) {
                    v.ultima.insert(k.clone(), ahora);
                }
            }
            v.contadores = nuevos;
        }
        // Estado de los reenvíos
        let Ok(Some(c)) = self.actual() else { return };
        let mut estados = HashMap::new();
        for r in c.reenvios.iter().filter(|r| r.activo) {
            let Some(ip) = ip_reenvio(r) else { continue };
            let ip = IpAddr::V4(ip);
            let conectado = vecinos.iter().any(|v| v.ip == ip && v.conectado);
            let estado = if !conectado {
                "desconectado"
            } else if r.protocolo.incluye_tcp() {
                let puerto = r.interno.unwrap_or(r.externo.first().map(|x| x.desde).unwrap_or(0));
                let s2 = s.clone();
                let ok = tokio::task::spawn_blocking(move || s2.conectar(ip, puerto)).await.unwrap_or(false);
                if ok { "funcionando" } else { "no_responde" }
            } else {
                "funcionando"
            };
            estados.insert(r.id, estado);
        }
        self.vivo.lock().unwrap().estados = estados;
    }

    /// Configuración en uso: la pendiente de confirmar o, si no hay, la confirmada.
    fn actual(&self) -> Result<Option<Config>, String> {
        match self.ap.almacen.cargar_pendiente()? {
            Some(c) => Ok(Some(c)),
            None => self.ap.almacen.cargar_confirmada(),
        }
    }

    /// IPv4 de la WAN leídas ahora mismo: `(tarjeta, ip, dinámica)`.
    fn ips_wan(&self, c: &Config) -> Vec<(String, IpAddr, bool)> {
        leer::ips_wan(c, &leer::interfaces(&*self.sistema))
    }

    fn fijas(&self, c: &Config) -> Fijas {
        leer::fijas(&*self.sistema, &self.rutas, c)
    }

    /// Valida, aplica (con vuelta atrás) y guarda como pendiente un cambio.
    async fn mutar<T>(
        &self,
        revision: Option<u64>,
        usuario: &str,
        resumen: &str,
        f: impl FnOnce(&mut Config) -> Result<T, Fallo>,
    ) -> Result<(T, EnEspera, Config), Fallo> {
        let _g = self.cerrojo.lock().await;
        let c = self
            .actual()
            .map_err(|e| Fallo::error(StatusCode::INTERNAL_SERVER_ERROR, &e))?
            .ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
        if revision.is_some_and(|r| r != c.revision) {
            return Err(Fallo::error(StatusCode::CONFLICT, CONFLICTO));
        }
        let mut nueva = c.clone();
        let t = f(&mut nueva)?;
        nueva.revision += 1;
        self.aplicar_nueva(&nueva, usuario, resumen).map(|e| (t, e, nueva))
    }

    fn aplicar_nueva(&self, nueva: &Config, usuario: &str, resumen: &str) -> Result<EnEspera, Fallo> {
        let fijas = self.fijas(nueva);
        let errores = validar::validar(nueva, &fijas);
        if !errores.is_empty() {
            return Err(Fallo::errores(errores));
        }
        let e = self
            .ap
            .aplicar(nueva, &fijas, usuario, resumen, true)
            .map_err(|e| Fallo::errores(vec![e]))?
            .expect("con confirmación siempre hay espera");
        *self.ultimas_fijas.lock().unwrap() = Some(fijas);
        crate::dominios::rellenar(nueva, &*self.sistema, &mut self.dominios.lock().unwrap());
        self.geo_si_toca(nueva);
        Ok(e)
    }

    /// Lanza la descarga de la base de países si una regla usa países y no hay base (o es de
    /// otro mes). Un fallo se reintenta a la hora. Al terminar, `revisar` ve que cambian las
    /// fijas y vuelve a aplicar.
    fn geo_si_toca(&self, c: &Config) {
        let ahora = self.sistema.ahora();
        if crate::geo::usados(c).is_empty() || !crate::geo::toca_actualizar(&self.rutas, ahora) {
            return;
        }
        {
            let mut g = self.geo.lock().unwrap();
            if g.descargando || (g.intento != 0 && ahora < g.intento + 3600) {
                return;
            }
            g.descargando = true;
            g.intento = ahora;
        }
        let (s, r, g) = (self.sistema.clone(), self.rutas.clone(), self.geo.clone());
        let Ok(h) = tokio::runtime::Handle::try_current() else { return };
        h.spawn_blocking(move || {
            let res = crate::geo::actualizar(&r, &*s);
            let mut g = g.lock().unwrap();
            g.descargando = false;
            g.error = res.err();
        });
    }

    fn avisos_geo(&self, c: &Config) -> Vec<String> {
        let mut a = crate::geo::avisos(c, &self.rutas);
        if let Some(e) = &self.geo.lock().unwrap().error
            && !a.is_empty()
        {
            a = vec![format!("No se pudo descargar la base de países ({e}); se reintenta en una hora.")];
        }
        a
    }

    fn pendiente_json(&self) -> Value {
        match self.ap.en_espera() {
            Some(e) => json!({ "id": e.id, "segundos": e.vence.saturating_sub(self.sistema.ahora()) }),
            None => Value::Null,
        }
    }

    fn aceptado(&self, e: &EnEspera, c: &Config, extra: Value) -> Response {
        let mut v = json!({
            "pendiente": { "id": e.id, "segundos": e.vence.saturating_sub(self.sistema.ahora()) },
            "revision": c.revision,
        });
        if let (Some(o), Value::Object(x)) = (v.as_object_mut(), extra) {
            o.extend(x);
        }
        (StatusCode::ACCEPTED, Json(v)).into_response()
    }
}

fn usuario(h: &HeaderMap) -> String {
    netctl_sdk::servidor::usuario(h).unwrap_or_else(|| "admin".into())
}

fn ips_wan_json(v: &[(String, IpAddr, bool)]) -> Value {
    json!(v.iter().map(|(iface, ip, dinamica)| json!({ "iface": iface, "ip": ip, "dinamica": dinamica })).collect::<Vec<_>>())
}

fn json_a<T: serde::de::DeserializeOwned>(v: Value) -> Result<T, Fallo> {
    serde_json::from_value(v).map_err(|e| Fallo::errores(vec![format!("Datos no válidos: {e}")]))
}

/// Direcciones «IP:puerto» desde fuera para un reenvío.
fn como_conectarse(c: &Config, r: &Reenvio) -> Vec<String> {
    let Some(p) = r.externo.first() else { return vec![] };
    let puerto = if p.desde == p.hasta { p.desde.to_string() } else { format!("{}-{}", p.desde, p.hasta) };
    leer::ips_externas(c).iter().map(|ip| format!("{ip}:{puerto}")).collect()
}

/// Las reglas fijas, para enseñarlas con candado.
fn fijas_json(f: &Fijas) -> Value {
    let desde_panel = if f.alcance_internet { "desde cualquier sitio (acceso por internet activado)" } else { "desde tu red y la VPN" };
    let mut v = vec![
        json!({ "id": "fija_panel", "nombre": "Panel de netctl", "descripcion": format!("Puerto {} (TCP) {desde_panel}", f.panel) }),
        json!({ "id": "fija_ssh", "nombre": "SSH", "descripcion": format!("Puerto {} (TCP) desde tu red y la VPN", f.ssh) }),
    ];
    for (i, p) in &f.wg {
        v.push(json!({ "id": format!("fija_wg_{i}"), "nombre": format!("WireGuard ({i})"), "descripcion": format!("Puerto {p} (UDP) desde cualquier sitio, para que conecte la VPN") }));
    }
    v.push(json!({ "id": "fija_red", "nombre": "Funcionamiento de la red", "descripcion": "DHCP e ICMP necesarios para que la máquina tenga IP e IPv6 funcione" }));
    Value::Array(v)
}

// ---------------------------------------------------------------- cuerpos

#[derive(Deserialize)]
struct ConRevision {
    #[serde(default)]
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct Evento {
    tipo: String,
    #[serde(default)]
    restaurar: bool,
}

#[derive(Deserialize)]
struct CuerpoAsistente {
    interfaces: BTreeMap<String, String>,
    #[serde(default = "si")]
    ping_externo: bool,
}

fn si() -> bool {
    true
}

#[derive(Deserialize)]
struct CuerpoZona {
    zona: String,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoPolitica {
    politica: Politica,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoRegla {
    regla: Value,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoOrden {
    ids: Vec<u64>,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoReenvio {
    reenvio: Value,
    #[serde(default)]
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoNat {
    nat: Value,
    #[serde(default)]
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoZonaNueva {
    #[serde(default)]
    nombre: String,
    #[serde(default)]
    tarjetas: Vec<String>,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoGrupo {
    #[serde(default)]
    tipo: String,
    nombre: String,
    #[serde(default)]
    miembros: Vec<Direccion>,
    #[serde(default)]
    puertos: Vec<RangoPuerto>,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoAjustes {
    ping_externo: Option<bool>,
    #[serde(default)]
    nat: BTreeMap<String, bool>,
    revision: Option<u64>,
}

#[derive(Deserialize)]
struct CuerpoConfig {
    config: Config,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct ConsultaRegistro {
    n: Option<usize>,
}

/// Lee un objeto JSON añadiéndole `id` (los cuerpos de alta no lo traen).
fn con_id<T: serde::de::DeserializeOwned>(mut v: Value, id: u64) -> Result<T, Fallo> {
    if let Some(o) = v.as_object_mut() {
        o.insert("id".into(), json!(id));
    }
    json_a(v)
}

// ---------------------------------------------------------------- rutas

type E = State<Arc<Estado>>;

pub fn router(e: Arc<Estado>) -> Router {
    Router::new()
        .route("/salud", get(|| async { "ok" }))
        .route("/resumen", get(resumen))
        .route("/evento", post(evento))
        .route("/equipos", get(equipos))
        .route("/asistente", get(sugerir).post(asistente))
        .route("/config", get(ver_config).put(poner_config))
        .route("/interfaces", get(interfaces))
        .route("/interfaces/{i}", put(zona_interfaz))
        .route("/politicas/{o}/{d}", get(ver_politica).put(poner_politica))
        .route("/reglas", get(reglas).post(crear_regla))
        .route("/reglas/orden", post(ordenar_reglas))
        .route("/reglas/{id}", put(editar_regla).delete(borrar_regla))
        .route("/reglas/{id}/duplicar", post(duplicar_regla))
        .route("/politicas", get(politicas))
        .route("/tabla", get(tabla))
        .route("/geo", get(ver_geo))
        .route("/zonas", post(crear_zona))
        .route("/zonas/{id}", put(editar_zona).delete(borrar_zona))
        .route("/reenvios", get(reenvios).post(crear_reenvio))
        .route("/reenvios/comprobar", post(comprobar_reenvio))
        .route("/reenvios/{id}", put(editar_reenvio).delete(borrar_reenvio))
        .route("/reenvios/{id}/{accion}", post(accion_reenvio))
        .route("/nat", get(nat_lista).post(crear_nat))
        .route("/nat/{id}", put(editar_nat).delete(borrar_nat))
        .route("/nat/{id}/{accion}", post(accion_nat))
        .route("/plantillas", get(|| async { Json(plantillas::todas()) }))
        .route("/grupos", get(grupos).post(crear_grupo))
        .route("/grupos/{id}", put(editar_grupo).delete(borrar_grupo))
        .route("/pendiente", get(pendiente))
        .route("/confirmar/{id}", post(confirmar))
        .route("/contadores", get(contadores))
        .route("/generada", get(generada))
        .route("/registro", get(registro))
        .route("/ajustes", put(ajustes))
        .with_state(e)
}

async fn resumen(State(e): E) -> Json<Value> {
    let Ok(Some(c)) = e.actual() else {
        return Json(json!({ "metricas": [
            { "etiqueta": "Firewall", "valor": "Sin configurar", "detalle": "Ábrelo para proteger la máquina en tres pasos" }
        ]}));
    };
    let v = e.vivo.lock().unwrap();
    let bloqueados: u64 = v
        .contadores
        .iter()
        .filter(|(k, _)| k.starts_with("p_"))
        .filter(|(k, _)| {
            let partes: Vec<&str> = k.trim_start_matches("p_").splitn(2, '_').collect();
            partes.len() == 2 && c.politica(partes[0], partes[1]) == Politica::Bloquear
        })
        .map(|(_, (p, _))| p)
        .sum();
    let problemas = v.estados.values().filter(|s| **s != "funcionando").count();
    let activos = c.reenvios.iter().filter(|r| r.activo).count();
    Json(json!({ "metricas": [
        { "etiqueta": "Reglas activas", "valor": c.reglas.iter().filter(|r| r.activa).count().to_string(), "detalle": format!("{} zonas", c.zonas.len()) },
        { "etiqueta": "Puertos abiertos", "valor": activos.to_string(),
          "detalle": if problemas > 0 { format!("{problemas} con problemas") } else { "Reenvíos de puertos".into() } },
        { "etiqueta": "Bloqueados", "valor": bloqueados.to_string(), "detalle": "Paquetes desde el último arranque" },
    ]}))
}

async fn evento(State(e): E, Json(ev): Json<Evento>) -> Json<Value> {
    let mensajes = match ev.tipo.as_str() {
        "modulos_cambiados" => {
            e.revisar().await;
            vec![]
        }
        "desinstalando" => {
            let _g = e.cerrojo.lock().await;
            let m = if ev.restaurar {
                aplicar::restaurar_anterior(&*e.sistema, &e.rutas)
            } else {
                aplicar::quitar_tabla(&*e.sistema);
                vec!["Tabla del firewall quitada.".into()]
            };
            e.ap.registro.anotar(Entrada {
                fecha: e.sistema.ahora(),
                usuario: "netctl".into(),
                resumen: format!("Desinstalando el módulo. {}", m.join(" ")),
                estado: EstadoCambio::Aplicado,
                detalle: None,
            });
            m
        }
        _ => vec![],
    };
    Json(json!({ "mensajes": mensajes }))
}

async fn equipos(State(e): E) -> Json<Value> {
    let v = leer::vecinos(&*e.sistema);
    // Nombres que el usuario ya puso a equipos en reglas, reenvíos y grupos.
    let mut nombres: HashMap<String, String> = HashMap::new();
    if let Ok(Some(c)) = e.actual() {
        let mut anotar = |d: &Direccion| {
            if let Direccion::Equipo { mac, nombre, .. } = d
                && !nombre.is_empty()
            {
                nombres.insert(mac.to_lowercase(), nombre.clone());
            }
        };
        c.reglas.iter().for_each(|r| {
            anotar(&r.origen);
            anotar(&r.destino);
        });
        c.reenvios.iter().for_each(|r| {
            anotar(&r.equipo);
            r.desde.iter().for_each(&mut anotar);
        });
        c.grupos_ip.iter().for_each(|g| g.miembros.iter().for_each(&mut anotar));
    }
    let l: Vec<Value> = v
        .iter()
        .map(|x| {
            let nombre = nombres.get(&x.mac).cloned().unwrap_or_default();
            json!({ "nombre": nombre, "ip": x.ip, "mac": x.mac, "red": x.iface, "conectado": x.conectado, "origen": "red" })
        })
        .collect();
    Json(Value::Array(l))
}

async fn sugerir(State(e): E) -> Json<Value> {
    let ifaces: Vec<Iface> = leer::interfaces(&*e.sistema).into_iter().filter(|i| i.nombre != "lo").collect();
    let sug = Config::por_defecto(&ifaces, leer::ruta_defecto(&e.rutas).as_deref());
    let l: Vec<Value> = ifaces
        .iter()
        .map(|i| json!({ "nombre": i.nombre, "ips": i.ips, "mac": i.mac, "activa": i.activa, "zona_sugerida": sug.interfaces.get(&i.nombre) }))
        .collect();
    let zonas: Vec<Value> = sug
        .zonas
        .iter()
        .filter(|z| z.tipo != TipoZona::Gateway)
        .map(|z| json!({ "id": z.id, "nombre": z.nombre, "tipo": z.tipo }))
        .collect();
    Json(json!({
        "interfaces": l,
        "zonas": zonas,
        "otro_firewall": aplicar::otro_firewall(&*e.sistema),
        "una_interfaz": sug.una_interfaz(),
        "fijas": fijas_json(&e.fijas(&sug)),
        "configurado": matches!(e.actual(), Ok(Some(_))),
    }))
}

async fn asistente(State(e): E, h: HeaderMap, Json(b): Json<CuerpoAsistente>) -> R {
    let ifaces: Vec<Iface> = leer::interfaces(&*e.sistema).into_iter().filter(|i| i.nombre != "lo").collect();
    let mut c = Config::por_defecto(&ifaces, leer::ruta_defecto(&e.rutas).as_deref());
    c.interfaces = b.interfaces;
    c.ping_externo = b.ping_externo;
    c.revision = 1;
    let errores = validar::validar(&c, &e.fijas(&c));
    if !errores.is_empty() {
        return Err(Fallo::errores(errores));
    }
    let _g = e.cerrojo.lock().await;
    if matches!(e.actual(), Ok(Some(_))) {
        return Err(Fallo::error(StatusCode::CONFLICT, "El firewall ya está configurado."));
    }
    let mensajes = aplicar::traspaso_inicial(&*e.sistema, &e.rutas);
    for m in &mensajes {
        e.ap.registro.anotar(Entrada { fecha: e.sistema.ahora(), usuario: usuario(&h), resumen: m.clone(), estado: EstadoCambio::Aplicado, detalle: None });
    }
    let espera = e.aplicar_nueva(&c, &usuario(&h), "Firewall configurado con el asistente")?;
    Ok(e.aceptado(&espera, &c, json!({ "mensajes": mensajes })))
}

async fn ver_config(State(e): E) -> R {
    let c = e.actual().map_err(|x| Fallo::error(StatusCode::INTERNAL_SERVER_ERROR, &x))?;
    Ok(match c {
        None => Json(json!({ "configurado": false, "pendiente": Value::Null })).into_response(),
        Some(c) => {
            let f = e.fijas(&c);
            let mut matriz = vec![];
            for o in &c.zonas {
                for d in &c.zonas {
                    if o.tipo == TipoZona::Gateway && d.tipo == TipoZona::Gateway {
                        continue;
                    }
                    let de_serie = politica_de_serie(o.tipo, d.tipo, o.id == d.id);
                    let politica = c.politica(&o.id, &d.id);
                    let reglas = c.reglas.iter().filter(|r| r.origen_zona == o.id && r.destino_zona == d.id).count();
                    matriz.push(json!({ "origen": o.id, "destino": d.id, "politica": politica, "de_serie": de_serie,
                        "cambiada": politica != de_serie, "reglas": reglas }));
                }
            }
            Json(json!({
                "configurado": true,
                "matriz": matriz,
                "config": c,
                "fijas": fijas_json(&f),
                "panel": f.panel,
                "pendiente": e.pendiente_json(),
                "una_interfaz": c.una_interfaz(),
                "ips_externas": ips_wan_json(&e.ips_wan(&c)),
                "avisos": nft::avisos(&c, e.sistema.ahora()).into_iter().chain(e.dominios.lock().unwrap().avisos.clone()).chain(e.avisos_geo(&c)).collect::<Vec<_>>(),
                "tarjetas_de_zona": c.zonas.iter().map(|z| (z.id.clone(), c.ifaces_de(&z.id))).collect::<std::collections::BTreeMap<_, _>>(),
            }))
            .into_response()
        }
    })
}

async fn poner_config(State(e): E, h: HeaderMap, Json(b): Json<CuerpoConfig>) -> R {
    let rev = b.config.revision;
    let (_, esp, c) = e
        .mutar(Some(rev), &usuario(&h), "Configuración completa reemplazada", |c| {
            *c = b.config;
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn interfaces(State(e): E) -> R {
    let c = e.actual().ok().flatten();
    let mut l: Vec<Value> = leer::interfaces(&*e.sistema)
        .into_iter()
        .filter(|i| i.nombre != "lo")
        .map(|i| {
            let zona = c.as_ref().and_then(|c| c.interfaces.get(&i.nombre).cloned());
            json!({ "nombre": i.nombre, "ips": i.ips, "mac": i.mac, "activa": i.activa, "zona": zona, "existe": true })
        })
        .collect();
    if let Some(c) = &c {
        for (i, z) in &c.interfaces {
            if !l.iter().any(|x| x["nombre"] == json!(i)) {
                l.push(json!({ "nombre": i, "ips": [], "mac": null, "activa": false, "zona": z, "existe": false }));
            }
        }
    }
    Ok(Json(json!({ "interfaces": l })).into_response())
}

async fn zona_interfaz(State(e): E, h: HeaderMap, Path(i): Path<String>, Json(b): Json<CuerpoZona>) -> R {
    if !validar::es_iface(&i) {
        return Err(Fallo::errores(vec!["Ese nombre de interfaz no es válido.".into()]));
    }
    let ifaces = leer::interfaces(&*e.sistema);
    let resumen = format!("Interfaz {i} a la zona «{}»", b.zona);
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), &resumen, |c| {
            c.interfaces.insert(i.clone(), b.zona.clone());
            if let Some(x) = ifaces.iter().find(|x| x.nombre == i) {
                c.direcciones.insert(i.clone(), x.ips.clone());
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn ver_politica(State(e): E, Path((o, d)): Path<(String, String)>) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    let (Some(zo), Some(zd)) = (c.zona(&o), c.zona(&d)) else {
        return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
    };
    let de_serie = politica_de_serie(zo.tipo, zd.tipo, o == d);
    let reglas: Vec<&Regla> = c.reglas.iter().filter(|r| r.origen_zona == o && r.destino_zona == d).collect();
    Ok(Json(json!({ "politica": c.politica(&o, &d), "de_serie": de_serie, "reglas": reglas })).into_response())
}

async fn poner_politica(State(e): E, h: HeaderMap, Path((o, d)): Path<(String, String)>, Json(b): Json<CuerpoPolitica>) -> R {
    let texto = if b.politica == Politica::Permitir { "Permitir" } else { "Bloquear" };
    let resumen = format!("Política {o} → {d}: {texto}");
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), &resumen, |c| {
            let (Some(zo), Some(zd)) = (c.zona(&o), c.zona(&d)) else {
                return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
            };
            let de_serie = politica_de_serie(zo.tipo, zd.tipo, o == d);
            c.politicas.retain(|p| !(p.origen == o && p.destino == d));
            if b.politica != de_serie {
                c.politicas.push(PoliticaPar { origen: o.clone(), destino: d.clone(), politica: b.politica });
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

/// Texto corto de una dirección (como `textoDireccion` del kit).
fn texto_dir(c: &Config, d: &Direccion) -> String {
    match d {
        Direccion::Cualquiera => "Cualquiera".into(),
        Direccion::Equipo { ip, nombre, .. } if !nombre.is_empty() => format!("{nombre} · {ip}"),
        Direccion::Equipo { ip, .. } => ip.to_string(),
        Direccion::Ip { red } if red.prefix_len() == red.max_prefix_len() => red.addr().to_string(),
        Direccion::Ip { red } => red.to_string(),
        Direccion::Rango { desde, hasta } => format!("{desde} – {hasta}"),
        Direccion::Grupo { id } => {
            let n = c.grupos_ip.iter().find(|g| g.id == *id).map(|g| g.nombre.clone()).unwrap_or_else(|| id.to_string());
            format!("Grupo «{n}»")
        }
        Direccion::Interfaz { iface } => format!("Tarjeta {iface}"),
        Direccion::Mac { mac } => format!("MAC {mac}"),
        Direccion::Dominio { nombres } => nombres.join(", "),
        Direccion::Pais { codigos } => format!("País {}", codigos.join(", ")),
    }
}

fn texto_puertos(c: &Config, p: &Puertos) -> String {
    match p {
        Puertos::Cualquiera => "Cualquiera".into(),
        Puertos::Lista(l) => l
            .iter()
            .map(|r| if r.desde == r.hasta { r.desde.to_string() } else { format!("{}-{}", r.desde, r.hasta) })
            .collect::<Vec<_>>()
            .join(", "),
        Puertos::Grupo(id) => {
            let n = c.grupos_puertos.iter().find(|g| g.id == *id).map(|g| g.nombre.clone()).unwrap_or_else(|| id.to_string());
            format!("Grupo «{n}»")
        }
    }
}

/// Lista de políticas de la página Zonas: las de serie agrupadas por acción y las propias con
/// sus textos para las columnas.
async fn politicas(State(e): E) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    let (mut permitir, mut bloquear) = (vec![], vec![]);
    for o in &c.zonas {
        for d in &c.zonas {
            if o.tipo == TipoZona::Gateway && d.tipo == TipoZona::Gateway {
                continue;
            }
            let par = json!({ "origen": o.id, "destino": d.id });
            if c.politica(&o.id, &d.id) == Politica::Permitir { permitir.push(par) } else { bloquear.push(par) }
        }
    }
    let nombre = |z: &str| c.zona(z).map(|z| z.nombre.clone()).unwrap_or_else(|| z.to_string());
    let propias: Vec<Value> = c
        .reglas
        .iter()
        .map(|r| {
            let mut v = serde_json::to_value(r).unwrap_or(Value::Null);
            if let Some(o) = v.as_object_mut() {
                o.insert("zona_origen_nombre".into(), json!(nombre(&r.origen_zona)));
                o.insert("zona_destino_nombre".into(), json!(nombre(&r.destino_zona)));
                o.insert("texto_origen".into(), json!(texto_dir(&c, &r.origen)));
                o.insert("texto_destino".into(), json!(texto_dir(&c, &r.destino)));
                o.insert("texto_puertos".into(), json!(texto_puertos(&c, &r.puertos)));
                o.insert("texto_puertos_origen".into(), json!(texto_puertos(&c, &r.puertos_origen)));
            }
            v
        })
        .collect();
    Ok(Json(json!({
        "de_serie": [
            { "nombre": "Permitir todo", "accion": "permitir", "parejas": permitir },
            { "nombre": "Bloquear todo", "accion": "bloquear", "parejas": bloquear },
        ],
        "propias": propias,
    }))
    .into_response())
}

fn texto_proto(p: Protocolo, n: u8) -> String {
    match p {
        Protocolo::Cualquiera => "Cualquiera".into(),
        Protocolo::Tcp => "TCP".into(),
        Protocolo::Udp => "UDP".into(),
        Protocolo::TcpUdp => "TCP/UDP".into(),
        Protocolo::Icmp => "ICMP".into(),
        Protocolo::Otro => format!("Otro ({n})"),
    }
}

/// Tabla de políticas: reglas propias, NAT propio, reenvíos y, al final, las de serie
/// agrupadas como en `/politicas`. Todas con las mismas columnas.
async fn tabla(State(e): E) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    let nombre = |z: &str| c.zona(z).map(|z| z.nombre.clone()).unwrap_or_else(|| z.to_string());
    let mut filas = vec![];
    for r in &c.reglas {
        let accion = match r.accion {
            Accion::Permitir => "Permitir",
            Accion::Bloquear => "Bloquear",
            Accion::Rechazar => "Rechazar",
        };
        filas.push(json!({
            "tipo": "firewall", "id": r.id, "nombre": r.nombre, "accion": accion,
            "protocolo": texto_proto(r.protocolo, r.numero_protocolo),
            "zona_origen": nombre(&r.origen_zona), "zona_destino": nombre(&r.destino_zona),
            "origen": texto_dir(&c, &r.origen), "destino": texto_dir(&c, &r.destino),
            "puerto_destino": texto_puertos(&c, &r.puertos), "interfaz": Value::Null,
            "activa": r.activa, "de_serie": false,
        }));
    }
    for n in &c.nat_propio {
        let (accion, destino) = match &n.tipo {
            TipoNat::Masquerade => ("Masquerade", texto_dir(&c, &n.destino)),
            TipoNat::Snat { .. } => ("SNAT", texto_dir(&c, &n.destino)),
            TipoNat::Dnat { ip, puerto } => {
                let a = match puerto {
                    Some(p) => format!("{ip}:{p}"),
                    None => ip.to_string(),
                };
                ("DNAT", format!("{} → {a}", texto_dir(&c, &n.destino)))
            }
        };
        filas.push(json!({
            "tipo": "nat", "id": n.id, "nombre": n.nombre, "accion": accion,
            "protocolo": texto_proto(n.protocolo, 0), "zona_origen": Value::Null, "zona_destino": Value::Null,
            "origen": texto_dir(&c, &n.origen), "destino": destino,
            "puerto_destino": texto_puertos(&c, &n.puertos), "interfaz": n.iface,
            "activa": n.activa, "de_serie": false,
        }));
    }
    let ahora = e.sistema.ahora();
    let wan = e.ips_wan(&c);
    let externa = c.zonas_tipo(TipoZona::Externa).first().map(|z| nombre(z));
    {
        let v = e.vivo.lock().unwrap();
        for r in &c.reenvios {
            let vista = vista_reenvio(&c, r, &v, ahora, &wan);
            let mut puertos = texto_puertos(&c, &Puertos::Lista(r.externo.clone()));
            if let Some(i) = r.interno {
                puertos = format!("{puertos} → {i}");
            }
            let origen = if r.desde.is_empty() {
                "Cualquiera".to_string()
            } else {
                r.desde.iter().map(|d| texto_dir(&c, d)).collect::<Vec<_>>().join(", ")
            };
            let interfaz = match (&r.wan, r.wan_ip) {
                (Some(w), Some(ip)) => Some(format!("{w} · {ip}")),
                (Some(w), None) => Some(w.clone()),
                (None, Some(ip)) => Some(ip.to_string()),
                (None, None) => None,
            };
            filas.push(json!({
                "tipo": "reenvio", "id": r.id, "nombre": r.nombre, "accion": "Reenviar",
                "protocolo": texto_proto(r.protocolo, 0), "zona_origen": externa, "zona_destino": Value::Null,
                "origen": origen, "destino": texto_dir(&c, &r.equipo), "puerto_destino": puertos,
                "interfaz": interfaz, "activa": r.activo, "de_serie": false,
                "estado": vista["estado"], "caduca_en": vista["caduca_en"], "avisos": vista["avisos"],
                "como_conectarse": vista["como_conectarse"],
            }));
        }
    }
    let (mut permitir, mut bloquear) = (vec![], vec![]);
    for o in &c.zonas {
        for d in &c.zonas {
            if o.tipo == TipoZona::Gateway && d.tipo == TipoZona::Gateway {
                continue;
            }
            let par = json!({ "origen": o.id, "destino": d.id });
            if c.politica(&o.id, &d.id) == Politica::Permitir { permitir.push(par) } else { bloquear.push(par) }
        }
    }
    for (id, n, accion, parejas) in [("serie_permitir", "Permitir todo", "Permitir", permitir), ("serie_bloquear", "Bloquear todo", "Bloquear", bloquear)] {
        if parejas.is_empty() {
            continue;
        }
        filas.push(json!({
            "tipo": "firewall", "id": id, "nombre": n, "accion": accion, "protocolo": "Cualquiera",
            "zona_origen": Value::Null, "zona_destino": Value::Null, "origen": "Cualquiera", "destino": "Cualquiera",
            "puerto_destino": "Cualquiera", "interfaz": Value::Null, "activa": true, "de_serie": true, "parejas": parejas,
        }));
    }
    Ok(Json(Value::Array(filas)).into_response())
}

/// Estado de la base de países y los países que se pueden elegir.
async fn ver_geo(State(e): E) -> Json<Value> {
    let fecha = crate::geo::fecha(&e.rutas);
    let descargando = e.geo.lock().unwrap().descargando;
    let estado = if descargando { "descargando" } else if fecha.is_some() { "lista" } else { "falta" };
    let paises: Vec<Value> = match crate::geo::base(&e.rutas) {
        Some(b) => b.keys().filter(|c| *c != "ZZ").map(|c| json!({ "codigo": c })).collect(),
        None => crate::geo::CODIGOS.split(' ').map(|c| json!({ "codigo": c })).collect(),
    };
    Json(json!({ "estado": estado, "fecha": fecha, "paises": paises, "atribucion": "IP Geolocation by DB-IP", "enlace": "https://db-ip.com" }))
}

async fn duplicar_regla(State(e): E, h: HeaderMap, Path(id): Path<String>, Json(b): Json<ConRevision>) -> R {
    let id: u64 = id.parse().map_err(|_| Fallo::error(StatusCode::FORBIDDEN, FIJA))?;
    let (nuevo, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Regla duplicada", |c| {
            let pos = c.reglas.iter().position(|x| x.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            let mut r = c.reglas[pos].clone();
            r.id = c.nuevo_id();
            r.nombre = format!("{} (copia)", r.nombre);
            r.activa = false;
            let nuevo = r.id;
            c.reglas.insert(pos + 1, r);
            Ok(nuevo)
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({ "id": nuevo })))
}

fn poner_tarjetas(c: &mut Config, zona: &str, tarjetas: &[String]) -> Result<(), Fallo> {
    if let Some(i) = tarjetas.iter().find(|i| !validar::es_iface(i)) {
        return Err(Fallo::errores(vec![format!("«{}» no es un nombre de tarjeta válido.", crate::nombre_seguro(i))]));
    }
    c.interfaces.retain(|i, z| z != zona || tarjetas.contains(i));
    for i in tarjetas {
        c.interfaces.insert(i.clone(), zona.to_string());
    }
    Ok(())
}

fn nombre_zona(n: &str) -> Result<String, Fallo> {
    let n = n.trim();
    if n.is_empty() {
        return Err(Fallo::errores(vec!["Ponle un nombre a la zona, por ejemplo «Cámaras».".into()]));
    }
    Ok(n.to_string())
}

async fn crear_zona(State(e): E, h: HeaderMap, Json(b): Json<CuerpoZonaNueva>) -> R {
    let nombre = nombre_zona(&b.nombre)?;
    let (id, esp, c) = e
        .mutar(b.revision, &usuario(&h), &format!("Zona nueva «{nombre}»"), |c| {
            let id = format!("z{}", c.nuevo_id());
            c.zonas.push(Zona { id: id.clone(), nombre: nombre.clone(), tipo: TipoZona::Personalizada });
            poner_tarjetas(c, &id, &b.tarjetas)?;
            Ok(id)
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({ "id": id })))
}

async fn editar_zona(State(e): E, h: HeaderMap, Path(id): Path<String>, Json(b): Json<CuerpoZonaNueva>) -> R {
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), &format!("Zona «{id}» cambiada"), |c| {
            let z = c.zonas.iter_mut().find(|z| z.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            if z.tipo == TipoZona::Personalizada {
                z.nombre = nombre_zona(&b.nombre)?;
            }
            poner_tarjetas(c, &id, &b.tarjetas)
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn borrar_zona(State(e): E, h: HeaderMap, Path(id): Path<String>, Query(q): Query<ConRevision>) -> R {
    let c0 = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    match c0.zona(&id) {
        None => return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE)),
        Some(z) if z.tipo != TipoZona::Personalizada => {
            return Err(Fallo::error(StatusCode::FORBIDDEN, "Las zonas de serie no se pueden borrar; solo puedes cambiar sus tarjetas."));
        }
        _ => {}
    }
    let (_, esp, c) = e
        .mutar(q.revision, &usuario(&h), &format!("Zona «{id}» borrada"), |c| {
            let usos: Vec<String> = c.reglas.iter().filter(|r| r.origen_zona == id || r.destino_zona == id).map(|r| r.nombre.clone()).collect();
            if !usos.is_empty() {
                return Err(Fallo(
                    StatusCode::CONFLICT,
                    json!({ "error": "Esta zona se usa en otras reglas: bórralas o cámbialas antes.", "usos": usos }),
                ));
            }
            c.zonas.retain(|z| z.id != id);
            c.interfaces.retain(|_, z| *z != id);
            c.politicas.retain(|p| p.origen != id && p.destino != id);
            c.nat.remove(&id);
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn reglas(State(e): E) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    Ok(Json(json!({ "reglas": c.reglas, "fijas": fijas_json(&e.fijas(&c)) })).into_response())
}

async fn crear_regla(State(e): E, h: HeaderMap, Json(b): Json<CuerpoRegla>) -> R {
    let (id, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Regla nueva", |c| {
            let id = c.nuevo_id();
            let r: Regla = con_id(b.regla, id)?;
            c.reglas.push(r);
            Ok(id)
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({ "id": id })))
}

async fn editar_regla(State(e): E, h: HeaderMap, Path(id): Path<String>, Json(b): Json<CuerpoRegla>) -> R {
    let id: u64 = id.parse().map_err(|_| Fallo::error(StatusCode::FORBIDDEN, FIJA))?;
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Regla editada", |c| {
            let r: Regla = con_id(b.regla, id)?;
            let x = c.reglas.iter_mut().find(|x| x.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            *x = r;
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn borrar_regla(State(e): E, h: HeaderMap, Path(id): Path<String>, Query(q): Query<ConRevision>) -> R {
    let id: u64 = id.parse().map_err(|_| Fallo::error(StatusCode::FORBIDDEN, FIJA))?;
    let (_, esp, c) = e
        .mutar(q.revision, &usuario(&h), "Regla borrada", |c| {
            let antes = c.reglas.len();
            c.reglas.retain(|r| r.id != id);
            if c.reglas.len() == antes {
                return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn ordenar_reglas(State(e): E, h: HeaderMap, Json(b): Json<CuerpoOrden>) -> R {
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Reglas reordenadas", |c| {
            let mut a: Vec<u64> = c.reglas.iter().map(|r| r.id).collect();
            let mut n = b.ids.clone();
            a.sort();
            n.sort();
            if a != n {
                return Err(Fallo::errores(vec!["La lista de reglas ha cambiado mientras ordenabas. Recarga.".into()]));
            }
            c.reglas.sort_by_key(|r| b.ids.iter().position(|x| *x == r.id));
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

/// Estado en llano de un reenvío para la lista.
fn vista_reenvio(c: &Config, r: &Reenvio, v: &Vivo, ahora: u64, wan: &[(String, IpAddr, bool)]) -> Value {
    let estado = if !r.activo || r.caduca.is_some_and(|x| x <= ahora) {
        "desactivada"
    } else {
        v.estados.get(&r.id).copied().unwrap_or("comprobando")
    };
    let clave = format!("f{}", r.id);
    let mut x = serde_json::to_value(r).unwrap_or(Value::Null);
    if let Some(o) = x.as_object_mut() {
        o.insert("estado".into(), json!(estado));
        o.insert("caduca_en".into(), json!(r.caduca.filter(|_| r.activo).map(|t| t.saturating_sub(ahora))));
        o.insert("conexiones".into(), json!(v.contadores.get(&clave).map(|x| x.0).unwrap_or(0)));
        o.insert("ultima".into(), json!(v.ultima.get(&clave)));
        o.insert("como_conectarse".into(), json!(como_conectarse(c, r)));
        o.insert("avisos".into(), json!(validar::avisos_reenvio(c, r, wan)));
    }
    x
}

async fn reenvios(State(e): E) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    let ahora = e.sistema.ahora();
    let wan = e.ips_wan(&c);
    let v = e.vivo.lock().unwrap();
    let l: Vec<Value> = c.reenvios.iter().map(|r| vista_reenvio(&c, r, &v, ahora, &wan)).collect();
    Ok(Json(json!({ "reenvios": l, "ips_externas": ips_wan_json(&wan), "una_interfaz": c.una_interfaz() })).into_response())
}

async fn crear_reenvio(State(e): E, h: HeaderMap, Json(b): Json<CuerpoReenvio>) -> R {
    let (r, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Reenvío de puerto nuevo", |c| {
            let id = c.nuevo_id();
            let r: Reenvio = con_id(b.reenvio, id)?;
            c.reenvios.push(r.clone());
            Ok(r)
        })
        .await?;
    let cc = como_conectarse(&c, &r);
    Ok(e.aceptado(&esp, &c, json!({ "id": r.id, "como_conectarse": cc })))
}

async fn editar_reenvio(State(e): E, h: HeaderMap, Path(id): Path<u64>, Json(b): Json<CuerpoReenvio>) -> R {
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Reenvío de puerto editado", |c| {
            let r: Reenvio = con_id(b.reenvio, id)?;
            let x = c.reenvios.iter_mut().find(|x| x.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            *x = r;
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn borrar_reenvio(State(e): E, h: HeaderMap, Path(id): Path<u64>, Query(q): Query<ConRevision>) -> R {
    let (_, esp, c) = e
        .mutar(q.revision, &usuario(&h), "Reenvío de puerto borrado", |c| {
            let antes = c.reenvios.len();
            c.reenvios.retain(|r| r.id != id);
            if c.reenvios.len() == antes {
                return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn accion_reenvio(State(e): E, h: HeaderMap, Path((id, accion)): Path<(u64, String)>, Json(b): Json<ConRevision>) -> R {
    let resumen = match accion.as_str() {
        "activar" => "Reenvío de puerto activado",
        "desactivar" => "Reenvío de puerto desactivado",
        "duplicar" => "Reenvío de puerto duplicado",
        _ => return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE)),
    };
    let (nuevo, esp, c) = e
        .mutar(b.revision, &usuario(&h), resumen, |c| {
            let i = c.reenvios.iter().position(|x| x.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            match accion.as_str() {
                "activar" => {
                    if let Some((con, otro)) = validar::choca_con(c, &c.reenvios[i]) {
                        return Err(Fallo(
                            StatusCode::CONFLICT,
                            json!({ "error": format!("No se puede reanudar: choca con «{otro}»."), "con": con }),
                        ));
                    }
                    c.reenvios[i].activo = true;
                    Ok(None)
                }
                "desactivar" => {
                    c.reenvios[i].activo = false;
                    Ok(None)
                }
                _ => {
                    let mut copia = c.reenvios[i].clone();
                    copia.id = c.nuevo_id();
                    copia.nombre = format!("{} (copia)", copia.nombre);
                    copia.activo = false;
                    let nid = copia.id;
                    c.reenvios.push(copia);
                    Ok(Some(nid))
                }
            }
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({ "id": nuevo })))
}

async fn comprobar_reenvio(State(e): E, Json(b): Json<CuerpoReenvio>) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    let id = b.reenvio.get("id").and_then(|x| x.as_u64()).unwrap_or(0);
    let r: Reenvio = con_id(b.reenvio, id)?;
    let f = e.fijas(&c);
    let errores = validar::choques_reenvio(&c, &f, &r);
    let siguiente = if errores.is_empty() {
        None
    } else {
        r.externo.first().and_then(|p| validar::siguiente_libre(&c, &f, r.protocolo, p.desde, Some(r.id)))
    };
    let externas = leer::ips_externas(&c);
    let wan_privada = !externas.is_empty() && externas.iter().all(|i| i.is_private() || (i.octets()[0] == 100 && (64..128).contains(&i.octets()[1])));
    Ok(Json(json!({
        "errores": errores,
        "avisos": validar::avisos_reenvio(&c, &r, &e.ips_wan(&c)),
        "siguiente_libre": siguiente,
        "como_conectarse": como_conectarse(&c, &r),
        "wan_privada": wan_privada,
    }))
    .into_response())
}

async fn nat_lista(State(e): E) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    Ok(Json(json!({ "nat": c.nat_propio, "automatico": c.nat })).into_response())
}

async fn crear_nat(State(e): E, h: HeaderMap, Json(b): Json<CuerpoNat>) -> R {
    let (id, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Regla NAT nueva", |c| {
            let id = c.nuevo_id();
            let n: ReglaNat = con_id(b.nat, id)?;
            c.nat_propio.push(n);
            Ok(id)
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({ "id": id })))
}

async fn editar_nat(State(e): E, h: HeaderMap, Path(id): Path<u64>, Json(b): Json<CuerpoNat>) -> R {
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Regla NAT editada", |c| {
            let n: ReglaNat = con_id(b.nat, id)?;
            let x = c.nat_propio.iter_mut().find(|x| x.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            *x = n;
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn borrar_nat(State(e): E, h: HeaderMap, Path(id): Path<u64>, Query(q): Query<ConRevision>) -> R {
    let (_, esp, c) = e
        .mutar(q.revision, &usuario(&h), "Regla NAT borrada", |c| {
            let antes = c.nat_propio.len();
            c.nat_propio.retain(|n| n.id != id);
            if c.nat_propio.len() == antes {
                return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn accion_nat(State(e): E, h: HeaderMap, Path((id, accion)): Path<(u64, String)>, Json(b): Json<ConRevision>) -> R {
    let (activa, resumen) = match accion.as_str() {
        "activar" => (true, "Regla NAT activada"),
        "desactivar" => (false, "Regla NAT desactivada"),
        _ => return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE)),
    };
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), resumen, |c| {
            let x = c.nat_propio.iter_mut().find(|x| x.id == id).ok_or_else(|| Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE))?;
            x.activa = activa;
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn grupos(State(e): E) -> R {
    let c = e.actual().ok().flatten().ok_or_else(|| Fallo::error(StatusCode::CONFLICT, SIN_CONFIG))?;
    let ip: Vec<Value> = c
        .grupos_ip
        .iter()
        .map(|g| json!({ "id": g.id, "nombre": g.nombre, "miembros": g.miembros, "usos": validar::usos_grupo(&c, g.id) }))
        .collect();
    let puertos: Vec<Value> = c
        .grupos_puertos
        .iter()
        .map(|g| json!({ "id": g.id, "nombre": g.nombre, "puertos": g.puertos, "usos": validar::usos_grupo(&c, g.id) }))
        .collect();
    Ok(Json(json!({ "ip": ip, "puertos": puertos })).into_response())
}

fn nombre_grupo(n: &str) -> Result<String, Fallo> {
    let n = n.trim();
    if n.is_empty() {
        return Err(Fallo::errores(vec!["Ponle un nombre al grupo, por ejemplo «Oficina».".into()]));
    }
    Ok(n.to_string())
}

async fn crear_grupo(State(e): E, h: HeaderMap, Json(b): Json<CuerpoGrupo>) -> R {
    let nombre = nombre_grupo(&b.nombre)?;
    let (id, esp, c) = e
        .mutar(b.revision, &usuario(&h), &format!("Grupo nuevo «{nombre}»"), |c| {
            let id = c.nuevo_id();
            match b.tipo.as_str() {
                "ip" => c.grupos_ip.push(GrupoIp { id, nombre: nombre.clone(), miembros: b.miembros }),
                "puertos" => c.grupos_puertos.push(GrupoPuertos { id, nombre: nombre.clone(), puertos: b.puertos }),
                _ => return Err(Fallo::errores(vec!["Elige si es un grupo de IPs o de puertos.".into()])),
            }
            Ok(id)
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({ "id": id })))
}

async fn editar_grupo(State(e): E, h: HeaderMap, Path(id): Path<u64>, Json(b): Json<CuerpoGrupo>) -> R {
    let nombre = nombre_grupo(&b.nombre)?;
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), &format!("Grupo «{nombre}» editado"), |c| {
            if let Some(g) = c.grupos_ip.iter_mut().find(|g| g.id == id) {
                g.nombre = nombre.clone();
                g.miembros = b.miembros;
            } else if let Some(g) = c.grupos_puertos.iter_mut().find(|g| g.id == id) {
                g.nombre = nombre.clone();
                g.puertos = b.puertos;
            } else {
                return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn borrar_grupo(State(e): E, h: HeaderMap, Path(id): Path<u64>, Query(q): Query<ConRevision>) -> R {
    let (_, esp, c) = e
        .mutar(q.revision, &usuario(&h), "Grupo borrado", |c| {
            let usos = validar::usos_grupo(c, id);
            if !usos.is_empty() {
                return Err(Fallo(
                    StatusCode::CONFLICT,
                    json!({ "error": "Este grupo se usa en otras reglas: quítalo de ellas antes de borrarlo.", "usos": usos }),
                ));
            }
            let antes = c.grupos_ip.len() + c.grupos_puertos.len();
            c.grupos_ip.retain(|g| g.id != id);
            c.grupos_puertos.retain(|g| g.id != id);
            if c.grupos_ip.len() + c.grupos_puertos.len() == antes {
                return Err(Fallo::error(StatusCode::NOT_FOUND, NO_EXISTE));
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}

async fn pendiente(State(e): E) -> Json<Value> {
    Json(json!({ "pendiente": e.pendiente_json() }))
}

async fn confirmar(State(e): E, h: HeaderMap, Path(id): Path<u64>) -> R {
    let _g = e.cerrojo.lock().await;
    if e.ap.confirmar(id, &usuario(&h)) {
        Ok(Json(json!({ "ok": true })).into_response())
    } else {
        Err(Fallo::error(StatusCode::NOT_FOUND, NO_CONFIRMABLE))
    }
}

async fn contadores(State(e): E) -> Json<Value> {
    let m = e
        .sistema
        .ejecutar("nft", &["-j", "list", "table", "inet", "netctl"], None)
        .map(|j| leer::parse_contadores(&j))
        .unwrap_or_default();
    let v: serde_json::Map<String, Value> =
        m.into_iter().map(|(k, (p, b))| (k, json!({ "paquetes": p, "bytes": b }))).collect();
    Json(Value::Object(v))
}

async fn generada(State(e): E) -> Json<Value> {
    let texto = match e.actual() {
        Ok(Some(c)) => nft::generar(&c, &e.fijas(&c), e.sistema.ahora()).unwrap_or_else(|errs| errs.join("\n")),
        _ => String::new(),
    };
    Json(json!({ "texto": texto }))
}

async fn registro(State(e): E, Query(q): Query<ConsultaRegistro>) -> Json<Value> {
    Json(json!({ "entradas": e.ap.registro.listar(q.n.unwrap_or(200)) }))
}

async fn ajustes(State(e): E, h: HeaderMap, Json(b): Json<CuerpoAjustes>) -> R {
    let (_, esp, c) = e
        .mutar(b.revision, &usuario(&h), "Ajustes generales del firewall", |c| {
            if let Some(p) = b.ping_externo {
                c.ping_externo = p;
            }
            for (z, v) in b.nat {
                if c.zona(&z).is_none() {
                    return Err(Fallo::errores(vec![format!("La zona «{z}» no existe.")]));
                }
                c.nat.insert(z, v);
            }
            Ok(())
        })
        .await?;
    Ok(e.aceptado(&esp, &c, json!({})))
}
