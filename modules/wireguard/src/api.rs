//! API HTTP del módulo (detrás del proxy del núcleo en `/api/m/wireguard/…`).
//!
//! Toda petición que cambia algo puede traer `revision`; si no coincide con la guardada,
//! otra pestaña cambió algo entre medias y se responde 409. Los errores de validación son 422
//! con `errores: [..]` (los textos de `Proyecto::validar`).

use std::convert::Infallible;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use axum::body::Bytes;
use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio::sync::{Mutex, watch};
use wg_modelo::{Cliente, Proyecto, Subred, claves, nombre_seguro};

use crate::Rutas;
use crate::aplicar::{self, Cambio, Sistema};
use crate::estado::{Almacen, Documento};
use crate::sugerencias;
use crate::vivo::{EstadoPeer, Vivo};

const CONFLICTO: &str =
    "Alguien ha cambiado la configuración a la vez. Recarga y vuelve a intentarlo.";
const SIN_SERVIDOR: &str = "Todavía no hay servidor WireGuard configurado.";
const NO_CLIENTE: &str = "Ese cliente ya no existe. Recarga la lista.";

pub struct Estado {
    pub rutas: Rutas,
    sistema: Arc<dyn Sistema>,
    almacen: Almacen,
    doc: Mutex<Option<Documento>>,
    /// El módulo Firewall está instalado (y entonces el NAT es cosa suya).
    firewall: AtomicBool,
    vivo: watch::Sender<Vivo>,
}

impl Estado {
    pub fn nuevo(rutas: Rutas, sistema: Arc<dyn Sistema>) -> anyhow::Result<Arc<Self>> {
        let almacen = Almacen::new(&rutas.datos);
        let doc = almacen.cargar().map_err(anyhow::Error::msg)?;
        // Estado inicial de los módulos: lo que hay instalado en disco. Luego lo actualiza el
        // núcleo con `POST /evento` (modulos_cambiados).
        let firewall = rutas
            .modulos
            .join("firewall")
            .join("manifest.toml")
            .exists();
        Ok(Arc::new(Self {
            rutas,
            sistema,
            almacen,
            doc: Mutex::new(doc),
            firewall: AtomicBool::new(firewall),
            vivo: watch::Sender::new(Vivo::default()),
        }))
    }

    pub fn firewall_presente(&self) -> bool {
        self.firewall.load(Ordering::SeqCst)
    }

    /// Una consulta a `wg show <iface> dump`.
    pub async fn sondear(&self) {
        let iface = self
            .doc
            .lock()
            .await
            .as_ref()
            .map(|d| d.proyecto.servidor.interfaz.trim().to_string());
        let Some(iface) = iface else { return };
        let r = self.sistema.ejecutar("wg", &["show", &iface, "dump"], None);
        let ahora = self.sistema.ahora();
        self.vivo
            .send_modify(|v| *v = v.siguiente(&iface, r, ahora));
    }

    /// Bucle de sondeo cada 5 s (lo lanza `main`).
    pub async fn sondear_siempre(self: Arc<Self>) {
        let mut t = tokio::time::interval(std::time::Duration::from_secs(
            crate::vivo::INTERVALO_SEGUNDOS,
        ));
        loop {
            t.tick().await;
            self.sondear().await;
        }
    }

    /// Al arrancar: adopta la interfaz y recarga las reglas nftables.
    pub async fn arrancar(&self) {
        let g = self.doc.lock().await;
        if let Some(d) = g.as_ref()
            && let Err(e) = aplicar::adoptar(
                &*self.sistema,
                &self.rutas,
                &d.proyecto,
                self.firewall_presente(),
            )
        {
            eprintln!("No se pudo aplicar la configuración al arrancar: {e}");
        }
    }

    /// Valida, guarda y aplica un cambio sobre el proyecto. Devuelve la revisión nueva.
    async fn mutar<F>(&self, revision: Option<u64>, f: F) -> Result<(u64, Cambio), Fallo>
    where
        F: FnOnce(&mut Proyecto) -> Result<Cambio, Fallo>,
    {
        let mut g = self.doc.lock().await;
        let d = g
            .as_mut()
            .ok_or_else(|| Fallo::new(StatusCode::CONFLICT, SIN_SERVIDOR))?;
        if revision.is_some_and(|r| r != d.revision) {
            return Err(Fallo::new(StatusCode::CONFLICT, CONFLICTO));
        }
        let iface_antes = d.proyecto.servidor.interfaz.trim().to_string();
        let mut p = d.proyecto.clone();
        let mut cambio = f(&mut p)?;
        validar(&p)?;
        let nuevo = Documento {
            revision: d.revision + 1,
            proyecto: p,
        };
        self.almacen.guardar(&nuevo).map_err(Fallo::interno)?;
        *d = nuevo;
        if d.proyecto.servidor.interfaz.trim() != iface_antes {
            aplicar::retirar(&*self.sistema, &self.rutas, &iface_antes);
            cambio = Cambio::Servidor;
        }
        let r = aplicar::aplicar(
            &*self.sistema,
            &self.rutas,
            &d.proyecto,
            cambio,
            self.firewall_presente(),
        );
        let rev = d.revision;
        drop(g);
        self.sondear().await;
        r.map_err(|e| no_aplicado(rev, &e))?;
        Ok((rev, cambio))
    }

    /// Sustituye el proyecto entero (asistente e importar).
    async fn reemplazar(&self, p: Proyecto, permitir: bool) -> Result<u64, Fallo> {
        validar(&p)?;
        let mut g = self.doc.lock().await;
        if g.is_some() && !permitir {
            return Err(Fallo::new(
                StatusCode::CONFLICT,
                "Ya hay un servidor configurado. Confirma que quieres reemplazarlo.",
            ));
        }
        let anterior = g
            .as_ref()
            .map(|d| (d.revision, d.proyecto.servidor.interfaz.trim().to_string()));
        let nuevo = Documento {
            revision: anterior.as_ref().map_or(1, |a| a.0 + 1),
            proyecto: p,
        };
        self.almacen.guardar(&nuevo).map_err(Fallo::interno)?;
        if let Some((_, iface)) = &anterior
            && iface != nuevo.proyecto.servidor.interfaz.trim()
        {
            aplicar::retirar(&*self.sistema, &self.rutas, iface);
        }
        let r = aplicar::aplicar(
            &*self.sistema,
            &self.rutas,
            &nuevo.proyecto,
            Cambio::Servidor,
            self.firewall_presente(),
        );
        let rev = nuevo.revision;
        *g = Some(nuevo);
        drop(g);
        self.sondear().await;
        r.map_err(|e| no_aplicado(rev, &e))?;
        Ok(rev)
    }
}

// ---------------------------------------------------------------- errores

pub struct Fallo(StatusCode, Value);

impl Fallo {
    fn new(estado: StatusCode, mensaje: &str) -> Self {
        Self(estado, json!({ "error": mensaje }))
    }
    fn interno(mensaje: String) -> Self {
        Self(
            StatusCode::INTERNAL_SERVER_ERROR,
            json!({ "error": mensaje }),
        )
    }
    fn invalido(mensaje: &str) -> Self {
        Self::validacion(vec![mensaje.to_string()])
    }
    fn validacion(errores: Vec<String>) -> Self {
        let error = format!("Revisa los datos: {}", errores[0]);
        Self(
            StatusCode::UNPROCESSABLE_ENTITY,
            json!({ "error": error, "errores": errores }),
        )
    }
}

impl IntoResponse for Fallo {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}

fn no_aplicado(revision: u64, e: &str) -> Fallo {
    Fallo(
        StatusCode::INTERNAL_SERVER_ERROR,
        json!({ "error": format!("Se guardó, pero no se pudo aplicar en el sistema: {e}"), "revision": revision }),
    )
}

fn validar(p: &Proyecto) -> Result<(), Fallo> {
    let e = p.validar();
    if e.is_empty() {
        Ok(())
    } else {
        Err(Fallo::validacion(e))
    }
}

// ---------------------------------------------------------------- vistas

#[derive(Serialize)]
struct ClienteVista {
    indice: usize,
    nombre: String,
    ip: String,
    activo: bool,
    internet: bool,
    toda_intranet: bool,
    subredes: Vec<u64>,
    extra: String,
    usar_dns: bool,
    usar_psk: bool,
    keepalive: String,
    clave_publica: String,
    /// Resumen legible: «internet, toda la intranet».
    acceso: String,
    estado: EstadoPeer,
}

fn vista_cliente(p: &Proyecto, i: usize, c: &Cliente, v: &Vivo) -> ClienteVista {
    let estado = c
        .activo
        .then(|| v.peer(c.clave_publica.trim()).cloned())
        .flatten()
        .unwrap_or_else(|| EstadoPeer {
            clave_publica: c.clave_publica.clone(),
            ..Default::default()
        });
    ClienteVista {
        indice: i,
        nombre: c.nombre.clone(),
        ip: c.ip.clone(),
        activo: c.activo,
        internet: c.internet,
        toda_intranet: c.toda_intranet,
        subredes: c.subredes.clone(),
        extra: c.extra.clone(),
        usar_dns: c.usar_dns,
        usar_psk: c.usar_psk,
        keepalive: c.keepalive.clone(),
        clave_publica: c.clave_publica.clone(),
        acceso: p.resumen_acceso(c),
        estado,
    }
}

/// Vivo solo cuenta si es de la interfaz actual (tras renombrarla, aún no hay muestra).
fn vivo_de<'a>(p: &Proyecto, v: &'a Vivo) -> Option<&'a Vivo> {
    (v.interfaz == p.servidor.interfaz.trim()).then_some(v)
}

#[derive(Serialize, Default)]
struct Metricas {
    conectados: usize,
    total: usize,
    activos: usize,
    rx: u64,
    tx: u64,
    tasa_rx: u64,
    tasa_tx: u64,
}

fn metricas(clientes: &[ClienteVista]) -> Metricas {
    let mut m = Metricas {
        total: clientes.len(),
        ..Default::default()
    };
    for c in clientes {
        m.activos += c.activo as usize;
        m.conectados += c.estado.conectado as usize;
        m.rx += c.estado.rx;
        m.tx += c.estado.tx;
        m.tasa_rx += c.estado.tasa_rx;
        m.tasa_tx += c.estado.tasa_tx;
    }
    m
}

fn vista_clientes(d: &Documento, v: &Vivo) -> Value {
    let p = &d.proyecto;
    let vivo = vivo_de(p, v);
    let vacio = Vivo::default();
    let fuente = vivo.unwrap_or(&vacio);
    let clientes: Vec<ClienteVista> = p
        .clientes
        .iter()
        .enumerate()
        .map(|(i, c)| vista_cliente(p, i, c, fuente))
        .collect();
    json!({
        "configurado": true,
        "revision": d.revision,
        "interfaz": p.servidor.interfaz.trim(),
        "interfaz_activa": vivo.is_some_and(|v| v.activa),
        "aviso": vivo.and_then(|v| v.aviso.clone()),
        "metricas": metricas(&clientes),
        "clientes": clientes,
        "intranet": p.intranet,
        "historial": vivo.map(|v| v.historial.clone()).unwrap_or_default(),
    })
}

#[derive(Serialize)]
struct ServidorVista<'a> {
    interfaz: &'a str,
    endpoint: &'a str,
    puerto: &'a str,
    red_vpn: &'a str,
    ip_servidor: &'a str,
    iface_salida: &'a str,
    dns: &'a str,
    mtu: &'a str,
    clave_publica: &'a str,
    nat: bool,
    firewall: bool,
    entre_clientes: bool,
}

/// `1536` → «1,5 KB» (igual que `formatoBytes` del kit).
pub fn formato_bytes(n: u64) -> String {
    const U: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < U.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    let t = if i > 0 && v < 10.0 {
        format!("{v:.1}")
    } else {
        format!("{v:.0}")
    };
    format!("{} {}", t.replace('.', ","), U[i])
}

// ---------------------------------------------------------------- entradas

#[derive(Deserialize, Default)]
#[serde(default)]
struct Rev {
    revision: Option<u64>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EntradaCliente {
    revision: Option<u64>,
    nombre: Option<String>,
    ip: Option<String>,
    internet: Option<bool>,
    toda_intranet: Option<bool>,
    subredes: Option<Vec<u64>>,
    extra: Option<String>,
    usar_dns: Option<bool>,
    usar_psk: Option<bool>,
    keepalive: Option<String>,
    activo: Option<bool>,
}

fn parchear_cliente(c: &mut Cliente, x: EntradaCliente) -> Result<(), Fallo> {
    if let Some(n) = x.nombre {
        if n.trim().is_empty() {
            return Err(Fallo::invalido(
                "El nombre del cliente no puede estar vacío",
            ));
        }
        c.nombre = n.trim().to_string();
    }
    if let Some(v) = x.ip {
        c.ip = v.trim().to_string();
    }
    if let Some(v) = x.internet {
        c.internet = v;
    }
    if let Some(v) = x.toda_intranet {
        c.toda_intranet = v;
    }
    if let Some(v) = x.subredes {
        c.subredes = v;
    }
    if let Some(v) = x.extra {
        c.extra = v.trim().to_string();
    }
    if let Some(v) = x.usar_dns {
        c.usar_dns = v;
    }
    if let Some(v) = x.usar_psk {
        c.usar_psk = v;
        if v && claves::publica_de(&c.psk).is_none() {
            c.psk = claves::generar_psk();
        }
    }
    if let Some(v) = x.keepalive {
        c.keepalive = v.trim().to_string();
    }
    if let Some(v) = x.activo {
        c.activo = v;
    }
    Ok(())
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct EntradaServidor {
    revision: Option<u64>,
    interfaz: Option<String>,
    endpoint: Option<String>,
    puerto: Option<String>,
    red_vpn: Option<String>,
    ip_servidor: Option<String>,
    iface_salida: Option<String>,
    dns: Option<String>,
    mtu: Option<String>,
    nat: Option<bool>,
    firewall: Option<bool>,
    entre_clientes: Option<bool>,
}

#[derive(Deserialize)]
struct EntradaSubred {
    #[serde(default)]
    revision: Option<u64>,
    nombre: Option<String>,
    cidr: Option<String>,
}

#[derive(Deserialize)]
struct EntradaAsistente {
    endpoint: String,
    #[serde(default = "puerto_defecto")]
    puerto: String,
    #[serde(default = "red_defecto")]
    red_vpn: String,
    iface_salida: String,
    #[serde(default)]
    dns: String,
    #[serde(default)]
    intranet: Vec<SubredNueva>,
    #[serde(default)]
    cliente: Option<ClienteNuevo>,
}

fn puerto_defecto() -> String {
    "51820".into()
}
fn red_defecto() -> String {
    "10.8.0.0/24".into()
}

#[derive(Deserialize)]
struct SubredNueva {
    nombre: String,
    cidr: String,
}

#[derive(Deserialize)]
struct ClienteNuevo {
    nombre: String,
    #[serde(default)]
    internet: bool,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct OpcionesImportar {
    reemplazar: bool,
}

#[derive(Deserialize)]
struct Evento {
    tipo: String,
    #[serde(default)]
    modulos: Vec<String>,
}

// ---------------------------------------------------------------- rutas

type E = State<Arc<Estado>>;

pub fn router(e: Arc<Estado>) -> Router {
    Router::new()
        .route("/salud", get(|| async { "ok" }))
        .route("/resumen", get(resumen))
        .route("/equipos", get(equipos))
        .route("/evento", post(evento))
        .route("/vivo", get(vivo))
        .route("/asistente", get(sugerir).post(asistente))
        .route("/importar", post(importar))
        .route("/servidor", get(servidor).put(editar_servidor))
        .route("/servidor/regenerar-claves", post(regenerar_servidor))
        .route("/clientes", get(clientes).post(crear_cliente))
        .route(
            "/clientes/{i}",
            get(cliente).put(editar_cliente).delete(borrar_cliente),
        )
        .route("/clientes/{i}/activar", post(activar))
        .route("/clientes/{i}/desactivar", post(desactivar))
        .route("/clientes/{i}/regenerar-claves", post(regenerar_cliente))
        .route("/clientes/{i}/conf", get(conf_cliente))
        .route("/clientes/{i}/qr.svg", get(qr_cliente))
        .route("/subredes", get(subredes).post(crear_subred))
        .route(
            "/subredes/{id}",
            axum::routing::put(editar_subred).delete(borrar_subred),
        )
        .with_state(e)
}

/// Clientes VPN para la lista de equipos del núcleo (selector de equipos).
async fn equipos(State(e): E) -> Json<Value> {
    let g = e.doc.lock().await;
    let Some(d) = g.as_ref() else { return Json(json!([])) };
    let v = e.vivo.borrow().clone();
    let vista = vista_clientes(d, &v);
    let l: Vec<Value> = vista["clientes"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|c| {
            json!({
                "nombre": c["nombre"],
                "ip": c["ip"].as_str().unwrap_or("").split('/').next().unwrap_or(""),
                "mac": "",
                "red": "VPN",
                "conectado": c["estado"]["conectado"],
                "desactivado": c["activo"] == json!(false),
                "origen": "vpn",
                "id": c["clave_publica"],
            })
        })
        .collect();
    Json(Value::Array(l))
}

async fn resumen(State(e): E) -> Json<Value> {
    let g = e.doc.lock().await;
    let Some(d) = g.as_ref() else {
        return Json(json!({ "metricas": [
            { "etiqueta": "Servidor", "valor": "Sin configurar", "detalle": "Ábrelo para crear la VPN en tres pasos" }
        ]}));
    };
    let v = e.vivo.borrow().clone();
    let vista = vista_clientes(d, &v);
    let m = &vista["metricas"];
    let conectados = m["conectados"].as_u64().unwrap_or(0);
    let mut primera = json!({
        "etiqueta": "Conectados",
        "valor": conectados.to_string(),
        "detalle": format!("de {} clientes", m["total"]),
    });
    if conectados > 0 {
        primera["color"] = "verde".into();
    }
    if let Some(a) = vista["aviso"].as_str() {
        primera["detalle"] = a.into();
    }
    Json(json!({ "metricas": [
        primera,
        { "etiqueta": "Recibido", "valor": formato_bytes(m["rx"].as_u64().unwrap_or(0)) },
        { "etiqueta": "Enviado", "valor": formato_bytes(m["tx"].as_u64().unwrap_or(0)) },
    ]}))
}

async fn evento(State(e): E, Json(ev): Json<Evento>) -> Json<Value> {
    if ev.tipo == "modulos_cambiados" {
        let fw = ev.modulos.iter().any(|m| m == "firewall");
        if e.firewall.swap(fw, Ordering::SeqCst) != fw
            && let Some(d) = e.doc.lock().await.as_ref()
            && let Err(x) = aplicar::solo_nft(&*e.sistema, &d.proyecto, fw)
        {
            eprintln!("No se pudo recargar la tabla nftables: {x}");
        }
    }
    Json(json!({ "ok": true }))
}

async fn vivo(State(e): E) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = e.vivo.subscribe();
    let flujo = futures_util::stream::unfold((rx, true), |(mut rx, primero)| async move {
        if !primero {
            rx.changed().await.ok()?;
        }
        let v = rx.borrow_and_update().clone();
        let ev = Event::default().json_data(&v).ok()?;
        Some((Ok(ev), (rx, false)))
    });
    Sse::new(flujo).keep_alive(KeepAlive::default())
}

async fn sugerir(State(e): E) -> Json<sugerencias::Sugerencias> {
    Json(sugerencias::calcular(&e.rutas))
}

async fn asistente(
    State(e): E,
    Json(x): Json<EntradaAsistente>,
) -> Result<(StatusCode, Json<Value>), Fallo> {
    let mut p = Proyecto::default();
    let s = &mut p.servidor;
    s.endpoint = x.endpoint.trim().into();
    s.puerto = x.puerto.trim().into();
    s.red_vpn = x.red_vpn.trim().into();
    s.iface_salida = x.iface_salida.trim().into();
    s.dns = x.dns.trim().into();
    p.servidor.ip_servidor = p
        .red_vpn()
        .ok()
        .and_then(|r| r.hosts().next())
        .map(|i| i.to_string())
        .unwrap_or_default();
    for sub in &x.intranet {
        if !sub.cidr.trim().is_empty() {
            let nombre = if sub.nombre.trim().is_empty() {
                "Intranet"
            } else {
                sub.nombre.trim()
            };
            p.anadir_subred(nombre, sub.cidr.trim());
        }
    }
    let mut indice = None;
    if let Some(c) = x.cliente.filter(|c| !c.nombre.trim().is_empty()) {
        let i = p.anadir_cliente();
        p.clientes[i].nombre = c.nombre.trim().into();
        p.clientes[i].internet = c.internet;
        indice = Some(i);
    }
    let rev = e.reemplazar(p, false).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "revision": rev, "indice": indice })),
    ))
}

async fn importar(
    State(e): E,
    Query(o): Query<OpcionesImportar>,
    cuerpo: Bytes,
) -> Result<Json<Value>, Fallo> {
    let p: Proyecto = serde_json::from_slice(&cuerpo).map_err(|x| {
        Fallo::new(
            StatusCode::BAD_REQUEST,
            &format!("El fichero no es un proyecto de wg-configurador válido: {x}"),
        )
    })?;
    let rev = e.reemplazar(p, o.reemplazar).await?;
    Ok(Json(json!({ "revision": rev })))
}

async fn servidor(State(e): E) -> Json<Value> {
    let g = e.doc.lock().await;
    let interfaces = sugerencias::calcular(&e.rutas).interfaces;
    let Some(d) = g.as_ref() else {
        return Json(json!({ "configurado": false, "interfaces": interfaces }));
    };
    let s = &d.proyecto.servidor;
    Json(json!({
        "configurado": true,
        "revision": d.revision,
        "servidor": ServidorVista {
            interfaz: &s.interfaz, endpoint: &s.endpoint, puerto: &s.puerto, red_vpn: &s.red_vpn,
            ip_servidor: &s.ip_servidor, iface_salida: &s.iface_salida, dns: &s.dns, mtu: &s.mtu,
            clave_publica: &s.clave_publica, nat: s.nat, firewall: s.firewall, entre_clientes: s.entre_clientes,
        },
        "intranet": d.proyecto.intranet,
        "clientes": d.proyecto.clientes.len(),
        "firewall_modulo": e.firewall_presente(),
        "interfaces": interfaces,
    }))
}

async fn editar_servidor(
    State(e): E,
    Json(x): Json<EntradaServidor>,
) -> Result<Json<Value>, Fallo> {
    let (rev, cambio) = e
        .mutar(x.revision, |p| {
            let s = &mut p.servidor;
            let antes = (
                s.interfaz.clone(),
                s.puerto.clone(),
                s.red_vpn.clone(),
                s.ip_servidor.clone(),
                s.mtu.clone(),
            );
            let t = |v: Option<String>, campo: &mut String| {
                if let Some(v) = v {
                    *campo = v.trim().to_string();
                }
            };
            t(x.interfaz, &mut s.interfaz);
            t(x.endpoint, &mut s.endpoint);
            t(x.puerto, &mut s.puerto);
            t(x.red_vpn, &mut s.red_vpn);
            t(x.ip_servidor, &mut s.ip_servidor);
            t(x.iface_salida, &mut s.iface_salida);
            t(x.dns, &mut s.dns);
            t(x.mtu, &mut s.mtu);
            if let Some(v) = x.nat {
                s.nat = v;
            }
            if let Some(v) = x.firewall {
                s.firewall = v;
            }
            if let Some(v) = x.entre_clientes {
                s.entre_clientes = v;
            }
            let despues = (
                s.interfaz.clone(),
                s.puerto.clone(),
                s.red_vpn.clone(),
                s.ip_servidor.clone(),
                s.mtu.clone(),
            );
            Ok(if antes != despues {
                Cambio::Servidor
            } else {
                Cambio::Peers
            })
        })
        .await?;
    Ok(Json(
        json!({ "revision": rev, "reinicio": cambio == Cambio::Servidor }),
    ))
}

async fn regenerar_servidor(State(e): E, x: Option<Json<Rev>>) -> Result<Json<Value>, Fallo> {
    let (rev, _) = e
        .mutar(x.unwrap_or_default().0.revision, |p| {
            let (privada, publica) = claves::generar_par();
            p.servidor.clave_privada = privada;
            p.servidor.clave_publica = publica;
            Ok(Cambio::Servidor)
        })
        .await?;
    Ok(Json(json!({ "revision": rev, "reinicio": true })))
}

async fn clientes(State(e): E) -> Json<Value> {
    let g = e.doc.lock().await;
    match g.as_ref() {
        None => Json(json!({ "configurado": false, "clientes": [] })),
        Some(d) => Json(vista_clientes(d, &e.vivo.borrow())),
    }
}

async fn cliente(State(e): E, Path(i): Path<usize>) -> Result<Json<Value>, Fallo> {
    let g = e.doc.lock().await;
    let d = g
        .as_ref()
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, SIN_SERVIDOR))?;
    let c = d
        .proyecto
        .clientes
        .get(i)
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, NO_CLIENTE))?;
    let v = e.vivo.borrow().clone();
    let vacio = Vivo::default();
    let vista = vista_cliente(
        &d.proyecto,
        i,
        c,
        vivo_de(&d.proyecto, &v).unwrap_or(&vacio),
    );
    let mut j = serde_json::to_value(vista).unwrap_or_default();
    j["revision"] = d.revision.into();
    Ok(Json(j))
}

fn con_cliente(p: &mut Proyecto, i: usize) -> Result<&mut Cliente, Fallo> {
    p.clientes
        .get_mut(i)
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, NO_CLIENTE))
}

async fn crear_cliente(
    State(e): E,
    Json(x): Json<EntradaCliente>,
) -> Result<(StatusCode, Json<Value>), Fallo> {
    let mut indice = 0;
    let (rev, _) = e
        .mutar(x.revision, |p| {
            indice = p.anadir_cliente();
            parchear_cliente(&mut p.clientes[indice], x)?;
            Ok(Cambio::Peers)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "revision": rev, "indice": indice })),
    ))
}

async fn editar_cliente(
    State(e): E,
    Path(i): Path<usize>,
    Json(x): Json<EntradaCliente>,
) -> Result<Json<Value>, Fallo> {
    let (rev, _) = e
        .mutar(x.revision, |p| {
            parchear_cliente(con_cliente(p, i)?, x)?;
            Ok(Cambio::Peers)
        })
        .await?;
    Ok(Json(json!({ "revision": rev })))
}

async fn borrar_cliente(
    State(e): E,
    Path(i): Path<usize>,
    Query(r): Query<Rev>,
) -> Result<Json<Value>, Fallo> {
    let (rev, _) = e
        .mutar(r.revision, |p| {
            con_cliente(p, i)?;
            p.clientes.remove(i);
            Ok(Cambio::Peers)
        })
        .await?;
    Ok(Json(json!({ "revision": rev })))
}

async fn fijar_activo(
    e: &Estado,
    i: usize,
    r: Option<Json<Rev>>,
    activo: bool,
) -> Result<Json<Value>, Fallo> {
    let (rev, _) = e
        .mutar(r.unwrap_or_default().0.revision, |p| {
            con_cliente(p, i)?.activo = activo;
            Ok(Cambio::Peers)
        })
        .await?;
    Ok(Json(json!({ "revision": rev })))
}

async fn activar(
    State(e): E,
    Path(i): Path<usize>,
    r: Option<Json<Rev>>,
) -> Result<Json<Value>, Fallo> {
    fijar_activo(&e, i, r, true).await
}

async fn desactivar(
    State(e): E,
    Path(i): Path<usize>,
    r: Option<Json<Rev>>,
) -> Result<Json<Value>, Fallo> {
    fijar_activo(&e, i, r, false).await
}

async fn regenerar_cliente(
    State(e): E,
    Path(i): Path<usize>,
    r: Option<Json<Rev>>,
) -> Result<Json<Value>, Fallo> {
    let (rev, _) = e
        .mutar(r.unwrap_or_default().0.revision, |p| {
            let c = con_cliente(p, i)?;
            let (privada, publica) = claves::generar_par();
            c.clave_privada = privada;
            c.clave_publica = publica;
            if c.usar_psk {
                c.psk = claves::generar_psk();
            }
            Ok(Cambio::Peers)
        })
        .await?;
    Ok(Json(json!({ "revision": rev })))
}

/// (nombre del fichero, contenido del .conf) de un cliente.
async fn conf_de(e: &Estado, i: usize) -> Result<(String, String), Fallo> {
    let g = e.doc.lock().await;
    let d = g
        .as_ref()
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, SIN_SERVIDOR))?;
    let c = d
        .proyecto
        .clientes
        .get(i)
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, NO_CLIENTE))?;
    let conf = d
        .proyecto
        .config_cliente(c)
        .map_err(|x| Fallo::invalido(&x))?;
    Ok((format!("{}.conf", nombre_seguro(&c.nombre)), conf))
}

async fn conf_cliente(State(e): E, Path(i): Path<usize>) -> Result<Response, Fallo> {
    let (nombre, conf) = conf_de(&e, i).await?;
    Ok((
        [
            (
                header::CONTENT_TYPE,
                "text/plain; charset=utf-8".to_string(),
            ),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename=\"{nombre}\""),
            ),
            // Lleva la clave privada del cliente: que no se quede en cachés.
            (header::CACHE_CONTROL, "no-store".to_string()),
        ],
        conf,
    )
        .into_response())
}

async fn qr_cliente(State(e): E, Path(i): Path<usize>) -> Result<Response, Fallo> {
    let (_, conf) = conf_de(&e, i).await?;
    let qr = qrcode::QrCode::with_error_correction_level(conf.as_bytes(), qrcode::EcLevel::M)
        .map_err(|x| Fallo::interno(format!("No se pudo generar el QR: {x}")))?;
    let svg = qr
        .render::<qrcode::render::svg::Color>()
        .min_dimensions(240, 240)
        .dark_color(qrcode::render::svg::Color("#000000"))
        .light_color(qrcode::render::svg::Color("#ffffff"))
        .build();
    Ok((
        [
            (header::CONTENT_TYPE, "image/svg+xml"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        svg,
    )
        .into_response())
}

async fn subredes(State(e): E) -> Result<Json<Value>, Fallo> {
    let g = e.doc.lock().await;
    let d = g
        .as_ref()
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, SIN_SERVIDOR))?;
    Ok(Json(
        json!({ "revision": d.revision, "subredes": d.proyecto.intranet }),
    ))
}

fn texto_obligatorio(v: Option<String>, que: &str) -> Result<Option<String>, Fallo> {
    match v.map(|t| t.trim().to_string()) {
        Some(t) if t.is_empty() => Err(Fallo::invalido(&format!("{que} no puede estar vacío"))),
        o => Ok(o),
    }
}

async fn crear_subred(
    State(e): E,
    Json(x): Json<EntradaSubred>,
) -> Result<(StatusCode, Json<Value>), Fallo> {
    let nombre = texto_obligatorio(x.nombre, "El nombre")?.unwrap_or_else(|| "Intranet".into());
    let cidr = texto_obligatorio(x.cidr, "La red")?
        .ok_or_else(|| Fallo::invalido("Falta la red (ej. 192.168.1.0/24)"))?;
    let mut id = 0;
    let (rev, _) = e
        .mutar(x.revision, |p| {
            p.anadir_subred(&nombre, &cidr);
            id = p.sig_id;
            Ok(Cambio::Peers)
        })
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "revision": rev, "id": id })),
    ))
}

fn con_subred(p: &mut Proyecto, id: u64) -> Result<&mut Subred, Fallo> {
    p.intranet
        .iter_mut()
        .find(|s| s.id == id)
        .ok_or_else(|| Fallo::new(StatusCode::NOT_FOUND, "Esa red ya no existe."))
}

async fn editar_subred(
    State(e): E,
    Path(id): Path<u64>,
    Json(x): Json<EntradaSubred>,
) -> Result<Json<Value>, Fallo> {
    let nombre = texto_obligatorio(x.nombre, "El nombre")?;
    let cidr = texto_obligatorio(x.cidr, "La red")?;
    let (rev, _) = e
        .mutar(x.revision, |p| {
            let s = con_subred(p, id)?;
            if let Some(n) = nombre {
                s.nombre = n;
            }
            if let Some(c) = cidr {
                s.cidr = c;
            }
            Ok(Cambio::Peers)
        })
        .await?;
    Ok(Json(json!({ "revision": rev })))
}

async fn borrar_subred(
    State(e): E,
    Path(id): Path<u64>,
    Query(r): Query<Rev>,
) -> Result<Json<Value>, Fallo> {
    let (rev, _) = e
        .mutar(r.revision, |p| {
            con_subred(p, id)?;
            p.intranet.retain(|s| s.id != id);
            for c in &mut p.clientes {
                c.subredes.retain(|s| *s != id);
            }
            Ok(Cambio::Peers)
        })
        .await?;
    Ok(Json(json!({ "revision": rev })))
}

#[cfg(test)]
mod tests {
    use super::formato_bytes;

    #[test]
    fn bytes_en_castellano() {
        assert_eq!(formato_bytes(0), "0 B");
        assert_eq!(formato_bytes(1536), "1,5 KB");
        assert_eq!(formato_bytes(2_254_857_830), "2,1 GB");
        assert_eq!(formato_bytes(40 * 1024 * 1024), "40 MB");
    }
}
