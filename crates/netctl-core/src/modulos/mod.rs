//! Gestor de módulos: instalados, catálogo, instalar/actualizar/desinstalar y avisos.

pub mod catalogo;
pub mod instalar;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use axum::extract::Path as RutaUrl;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use minisign_verify::PublicKey;
use netctl_sdk::distro::Familia;
use netctl_sdk::manifiesto::{EntradaMenu, Manifiesto};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::api::{Compartido, error};
use catalogo::{Catalogo, ErrorCatalogo};
use instalar::{Entorno, ErrorInstalar};

/// Raíces de las rutas del sistema (cambiables en tests).
#[derive(Clone, Debug)]
pub struct Rutas {
    /// `/usr/lib/netctl/modules`
    pub modulos: PathBuf,
    /// `/etc/systemd/system`
    pub unidades: PathBuf,
    /// `/run/netctl`
    pub run: PathBuf,
    /// `/var/lib/netctl`
    pub datos: PathBuf,
}

impl Rutas {
    pub fn sistema(datos: &Path) -> Self {
        Self {
            modulos: "/usr/lib/netctl/modules".into(),
            unidades: "/etc/systemd/system".into(),
            run: "/run/netctl".into(),
            datos: datos.to_path_buf(),
        }
    }

    pub fn dir_modulo(&self, id: &str) -> PathBuf {
        self.modulos.join(id)
    }

    pub fn binario(&self, id: &str) -> PathBuf {
        self.dir_modulo(id)
            .join("bin")
            .join(format!("netctl-mod-{id}"))
    }

    pub fn socket(&self, id: &str) -> PathBuf {
        self.run.join(format!("mod-{id}.sock"))
    }

    pub fn unidad(&self, id: &str) -> PathBuf {
        self.unidades.join(nombre_unidad(id))
    }
}

pub fn nombre_unidad(id: &str) -> String {
    format!("netctl-mod-{id}.service")
}

/// Mismo criterio que el manifiesto: minúsculas, cifras y guiones, hasta 32.
pub fn id_valido(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 32
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct EstadoUnidad {
    /// `active`, `inactive`, `failed`…
    pub activo: String,
    pub cpu_ns: Option<u64>,
    pub memoria: Option<u64>,
}

/// systemctl y el gestor de paquetes, detrás de un trait para poder probar sin root.
pub trait Systemd: Send + Sync {
    fn daemon_reload(&self) -> Result<()>;
    fn enable_now(&self, unidad: &str) -> Result<()>;
    fn disable_now(&self, unidad: &str) -> Result<()>;
    fn restart(&self, unidad: &str) -> Result<()>;
    fn estado(&self, unidad: &str) -> EstadoUnidad;
    fn instalar_paquetes(&self, f: Familia, paquetes: &[String]) -> Result<()> {
        netctl_sdk::distro::instalar_paquetes(f, paquetes)
    }
}

pub struct SystemdReal;

fn systemctl(args: &[&str]) -> Result<String> {
    let s = std::process::Command::new("systemctl")
        .args(args)
        .output()
        .context("No se pudo ejecutar systemctl")?;
    if !s.status.success() {
        bail!(
            "systemctl {} falló: {}",
            args.join(" "),
            String::from_utf8_lossy(&s.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&s.stdout).into_owned())
}

/// Interpreta la salida de `systemctl show -p ActiveState,CPUUsageNSec,MemoryCurrent`.
pub fn parsear_show(salida: &str) -> EstadoUnidad {
    let mut e = EstadoUnidad::default();
    for (k, v) in salida.lines().filter_map(|l| l.split_once('=')) {
        match k {
            "ActiveState" => e.activo = v.to_string(),
            "CPUUsageNSec" => e.cpu_ns = v.parse().ok(),
            "MemoryCurrent" => e.memoria = v.parse().ok(),
            _ => {}
        }
    }
    e
}

impl Systemd for SystemdReal {
    fn daemon_reload(&self) -> Result<()> {
        systemctl(&["daemon-reload"]).map(drop)
    }
    fn enable_now(&self, unidad: &str) -> Result<()> {
        systemctl(&["enable", "--now", unidad]).map(drop)
    }
    fn disable_now(&self, unidad: &str) -> Result<()> {
        systemctl(&["disable", "--now", unidad]).map(drop)
    }
    fn restart(&self, unidad: &str) -> Result<()> {
        systemctl(&["restart", unidad]).map(drop)
    }
    fn estado(&self, unidad: &str) -> EstadoUnidad {
        systemctl(&[
            "show",
            unidad,
            "-p",
            "ActiveState,CPUUsageNSec,MemoryCurrent",
        ])
        .map(|s| parsear_show(&s))
        .unwrap_or_else(|_| EstadoUnidad {
            activo: "desconocido".into(),
            ..Default::default()
        })
    }
}

#[derive(Serialize, Clone, Debug)]
pub struct InfoModulo {
    pub id: String,
    pub nombre: String,
    pub version: semver::Version,
    pub icono: String,
    pub menu: Vec<EntradaMenuJson>,
    #[serde(flatten)]
    pub estado: EstadoUnidad,
}

#[derive(Serialize, Clone, Debug)]
pub struct EntradaMenuJson {
    pub titulo: String,
    pub ruta: String,
    pub icono: String,
}

impl From<&EntradaMenu> for EntradaMenuJson {
    fn from(e: &EntradaMenu) -> Self {
        Self {
            titulo: e.titulo.clone(),
            ruta: e.ruta.clone(),
            icono: e.icono.clone(),
        }
    }
}

pub struct Gestor {
    pub entorno: Entorno,
    pub base_url: String,
    pub clave: Option<PublicKey>,
    pub familia: Option<Familia>,
    cerrojo: tokio::sync::Mutex<()>,
}

impl Gestor {
    pub fn new(
        rutas: Rutas,
        systemd: Arc<dyn Systemd>,
        base_url: &str,
        clave: Option<PublicKey>,
        familia: Option<Familia>,
    ) -> Self {
        let cliente = catalogo::cliente_http();
        let base_url = if base_url.ends_with('/') {
            base_url.to_string()
        } else {
            format!("{base_url}/")
        };
        Self {
            entorno: Entorno {
                rutas,
                systemd,
                cliente,
                espera_salud: Duration::from_secs(15),
            },
            base_url,
            clave,
            familia,
            cerrojo: tokio::sync::Mutex::new(()),
        }
    }

    pub fn rutas(&self) -> &Rutas {
        &self.entorno.rutas
    }

    /// Manifiestos de los módulos instalados (ordenados por id).
    pub fn instalados(&self) -> Vec<Manifiesto> {
        let Ok(dir) = std::fs::read_dir(&self.rutas().modulos) else {
            return vec![];
        };
        let mut v: Vec<Manifiesto> = dir
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_str().is_some_and(id_valido))
            .filter_map(|e| std::fs::read_to_string(e.path().join("manifest.toml")).ok())
            .filter_map(|t| Manifiesto::leer(&t).ok())
            .collect();
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    }

    pub async fn listar(&self) -> Vec<InfoModulo> {
        let mut v = Vec::new();
        for m in self.instalados() {
            let sd = self.entorno.systemd.clone();
            let unidad = nombre_unidad(&m.id);
            let estado = tokio::task::spawn_blocking(move || sd.estado(&unidad))
                .await
                .unwrap_or_default();
            v.push(InfoModulo {
                id: m.id.clone(),
                nombre: m.nombre.clone(),
                version: m.version.clone(),
                icono: m.icono.clone(),
                menu: m.menu.iter().map(Into::into).collect(),
                estado,
            });
        }
        v
    }

    pub async fn catalogo(&self) -> Result<Catalogo, ErrorCatalogo> {
        let clave = self.clave.as_ref().ok_or(ErrorCatalogo::SinClave)?;
        catalogo::obtener(&self.entorno.cliente, &self.base_url, clave).await
    }

    /// Instala o actualiza `id` desde el catálogo y avisa a los demás módulos.
    pub async fn instalar(&self, id: &str) -> Result<(), ErrorGestor> {
        let _g = self.cerrojo.try_lock().map_err(|_| ErrorGestor::Ocupado)?;
        let cat = self.catalogo().await.map_err(ErrorGestor::Catalogo)?;
        let entrada = cat
            .modulos
            .iter()
            .find(|e| e.id == id)
            .ok_or(ErrorGestor::NoExiste)?;
        let clave = self
            .clave
            .as_ref()
            .ok_or(ErrorGestor::Catalogo(ErrorCatalogo::SinClave))?;
        instalar::instalar(&self.entorno, entrada, clave, self.familia)
            .await
            .map_err(ErrorGestor::Instalar)?;
        self.notificar().await;
        Ok(())
    }

    /// `restaurar`: el módulo deshace lo que cambió en el sistema (el Firewall vuelve a
    /// activar firewalld/ufw). Se le avisa con `POST /evento {"tipo":"desinstalando"}` antes
    /// de pararlo; si no responde en 10 s, se sigue igualmente.
    pub async fn desinstalar(&self, id: &str, borrar_datos: bool, restaurar: bool) -> Result<(), ErrorGestor> {
        let _g = self.cerrojo.try_lock().map_err(|_| ErrorGestor::Ocupado)?;
        if !self.instalados().iter().any(|m| m.id == id) {
            return Err(ErrorGestor::NoExiste);
        }
        let cuerpo = json!({ "tipo": "desinstalando", "restaurar": restaurar }).to_string();
        let req = axum::http::Request::post("/evento")
            .header("host", "localhost")
            .header("content-type", "application/json")
            .body(axum::body::Body::from(cuerpo))
            .unwrap();
        let r = tokio::time::timeout(Duration::from_secs(10), crate::proxy::enviar(&self.rutas().socket(id), req)).await;
        if !matches!(r, Ok(Ok(ref resp)) if resp.status().is_success()) {
            // Módulo colgado o parado: se le pide lo mismo ejecutando su binario
            // (`netctl-mod-<id> desinstalar [--restaurar]`). Si no lo entiende, da igual.
            eprintln!("Aviso: el módulo {id} no respondió al aviso de desinstalación; se usa su programa");
            let mut orden = tokio::process::Command::new(self.rutas().binario(id));
            orden.arg("desinstalar");
            if restaurar {
                orden.arg("--restaurar");
            }
            match tokio::time::timeout(Duration::from_secs(20), orden.status()).await {
                Ok(Ok(st)) if st.success() => {}
                otro => eprintln!("Aviso: «netctl-mod-{id} desinstalar» no terminó bien: {otro:?}"),
            }
        }
        instalar::desinstalar(&self.entorno, id, borrar_datos)
            .await
            .map_err(ErrorGestor::Instalar)?;
        self.notificar().await;
        Ok(())
    }

    /// Equipos de la red: `extra` (vecinos que ve el núcleo) más los `GET /equipos` de cada
    /// módulo (el que no lo tenga se ignora), sin repetidos.
    pub async fn equipos(&self, extra: Vec<serde_json::Value>) -> Vec<serde_json::Value> {
        let mut listas = vec![extra];
        for m in self.instalados() {
            let req = axum::http::Request::get("/equipos")
                .header("host", "localhost")
                .body(axum::body::Body::empty())
                .unwrap();
            let r = tokio::time::timeout(Duration::from_secs(3), crate::proxy::enviar(&self.rutas().socket(&m.id), req)).await;
            let Ok(Ok(resp)) = r else { continue };
            if !resp.status().is_success() {
                continue;
            }
            let Ok(b) = http_body_util::BodyExt::collect(resp.into_body()).await else { continue };
            if let Ok(serde_json::Value::Array(l)) = serde_json::from_slice(&b.to_bytes()) {
                listas.push(l);
            }
        }
        juntar_equipos(listas)
    }

    /// `POST /evento {"tipo":"modulos_cambiados","modulos":[ids]}` a cada módulo instalado.
    pub async fn notificar(&self) {
        let ids: Vec<String> = self.instalados().into_iter().map(|m| m.id).collect();
        let cuerpo = json!({ "tipo": "modulos_cambiados", "modulos": ids }).to_string();
        for id in &ids {
            let req = axum::http::Request::post("/evento")
                .header("host", "localhost")
                .header("content-type", "application/json")
                .body(axum::body::Body::from(cuerpo.clone()))
                .unwrap();
            let r = tokio::time::timeout(
                Duration::from_secs(5),
                crate::proxy::enviar(&self.rutas().socket(id), req),
            )
            .await;
            if !matches!(r, Ok(Ok(ref resp)) if resp.status().is_success()) {
                eprintln!("Aviso: el módulo {id} no recibió el evento modulos_cambiados");
            }
        }
    }
}

#[derive(Debug)]
pub enum ErrorGestor {
    Ocupado,
    NoExiste,
    Catalogo(ErrorCatalogo),
    Instalar(ErrorInstalar),
}

impl IntoResponse for ErrorGestor {
    fn into_response(self) -> Response {
        match self {
            Self::Ocupado => error(
                StatusCode::CONFLICT,
                "Ya hay una instalación en curso; espera a que termine",
            ),
            Self::NoExiste => error(StatusCode::NOT_FOUND, "Ese módulo no existe"),
            Self::Catalogo(e) => error(StatusCode::BAD_GATEWAY, e.to_string()),
            Self::Instalar(e @ ErrorInstalar::NucleoAntiguo { .. }) => {
                error(StatusCode::CONFLICT, e.to_string())
            }
            Self::Instalar(
                e @ (ErrorInstalar::Descarga(_) | ErrorInstalar::Firma | ErrorInstalar::Sha256),
            ) => error(StatusCode::BAD_GATEWAY, e.to_string()),
            Self::Instalar(e) => error(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
        }
    }
}

type G = Extension<Arc<Gestor>>;

async fn listar(Extension(g): G) -> Response {
    Json(json!({ "modulos": g.listar().await })).into_response()
}

async fn ver_catalogo(Extension(g): G) -> Response {
    match g.catalogo().await {
        Ok(c) => {
            let instalados = g.instalados();
            let modulos: Vec<_> = c
                .modulos
                .iter()
                .map(|e| {
                    let inst = instalados
                        .iter()
                        .find(|m| m.id == e.id)
                        .map(|m| m.version.clone());
                    let actualizable = inst.as_ref().is_some_and(|v| *v < e.version);
                    json!({
                        "id": e.id, "nombre": e.nombre, "descripcion": e.descripcion,
                        "version": e.version, "nucleo_minimo": e.nucleo_minimo,
                        "instalado": inst, "actualizable": actualizable,
                        "compatible": e.nucleo_minimo <= instalar::version_nucleo(),
                    })
                })
                .collect();
            let actual = instalar::version_nucleo();
            let nucleo_nuevo = c.nucleo.as_ref().is_some_and(|v| *v > actual);
            Json(json!({ "modulos": modulos, "nucleo": c.nucleo, "nucleo_actual": actual, "nucleo_nuevo": nucleo_nuevo }))
                .into_response()
        }
        Err(e) => error(StatusCode::BAD_GATEWAY, e.to_string()),
    }
}

async fn instalar_ruta(Extension(g): G, RutaUrl(id): RutaUrl<String>) -> Response {
    if !id_valido(&id) {
        return ErrorGestor::NoExiste.into_response();
    }
    match g.instalar(&id).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Deserialize, Default)]
struct PeticionDesinstalar {
    #[serde(default)]
    borrar_datos: bool,
    #[serde(default)]
    restaurar: bool,
}

/// Junta listas de equipos sin repetir (por MAC; sin MAC, por IP). De los repetidos se queda
/// con el primer nombre no vacío y «conectado» si alguno lo está. Los conectados van primero.
pub fn juntar_equipos(listas: Vec<Vec<serde_json::Value>>) -> Vec<serde_json::Value> {
    let mut v: Vec<serde_json::Value> = vec![];
    for l in listas {
        for mut e in l {
            let mac = e["mac"].as_str().unwrap_or("").to_lowercase();
            e["mac"] = json!(mac);
            let clave = |x: &serde_json::Value| {
                let m = x["mac"].as_str().unwrap_or("");
                if m.is_empty() { format!("ip:{}", x["ip"].as_str().unwrap_or("")) } else { m.to_string() }
            };
            let k = clave(&e);
            match v.iter_mut().find(|x| clave(x) == k) {
                Some(x) => {
                    if x["nombre"].as_str().unwrap_or("").is_empty() && !e["nombre"].as_str().unwrap_or("").is_empty() {
                        x["nombre"] = e["nombre"].clone();
                    }
                    if e["conectado"] == json!(true) {
                        x["conectado"] = json!(true);
                        x["ip"] = e["ip"].clone();
                    }
                }
                None => v.push(e),
            }
        }
    }
    v.sort_by_key(|e| (e["conectado"] != json!(true), e["ip"].as_str().unwrap_or("").to_string()));
    v
}

/// `ip -j neigh` → equipos (solo los que tienen MAC).
pub fn vecinos_de_json(j: &str) -> Vec<serde_json::Value> {
    let Ok(serde_json::Value::Array(l)) = serde_json::from_str::<serde_json::Value>(j) else { return vec![] };
    l.iter()
        .filter_map(|n| {
            let ip = n["dst"].as_str()?;
            let mac = n["lladdr"].as_str()?.to_lowercase();
            let estados: Vec<&str> = n["state"].as_array().map(|a| a.iter().filter_map(|x| x.as_str()).collect()).unwrap_or_default();
            let conectado = estados.iter().any(|e| matches!(*e, "REACHABLE" | "DELAY" | "PROBE" | "PERMANENT"));
            Some(json!({ "nombre": "", "ip": ip, "mac": mac, "red": n["dev"].as_str().unwrap_or(""), "conectado": conectado, "origen": "red" }))
        })
        .collect()
}

async fn equipos_ruta(Extension(g): G) -> Response {
    let salida = tokio::process::Command::new("ip").args(["-j", "neigh"]).output().await;
    let propios = salida.ok().filter(|o| o.status.success()).map(|o| vecinos_de_json(&String::from_utf8_lossy(&o.stdout))).unwrap_or_default();
    Json(g.equipos(propios).await).into_response()
}

async fn desinstalar_ruta(
    Extension(g): G,
    RutaUrl(id): RutaUrl<String>,
    cuerpo: Option<Json<PeticionDesinstalar>>,
) -> Response {
    if !id_valido(&id) {
        return ErrorGestor::NoExiste.into_response();
    }
    let p = cuerpo.map(|Json(p)| p).unwrap_or_default();
    match g.desinstalar(&id, p.borrar_datos, p.restaurar).await {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(e) => e.into_response(),
    }
}

/// Rutas de gestión (requieren sesión: se montan en `api::router` como protegidas).
pub fn rutas(g: Arc<Gestor>) -> Router<Compartido> {
    Router::new()
        .route("/api/modulos", get(listar))
        .route("/api/equipos", get(equipos_ruta))
        .route("/api/catalogo", get(ver_catalogo))
        .route("/api/modulos/{id}/instalar", post(instalar_ruta))
        .route("/api/modulos/{id}/actualizar", post(instalar_ruta))
        .route("/api/modulos/{id}/desinstalar", post(desinstalar_ruta))
        .layer(Extension(g))
}

#[cfg(test)]
pub(crate) mod pruebas {
    //! Piezas comunes de los tests de módulos.
    use super::*;
    use std::sync::Mutex;

    pub struct Claves {
        pub publica: PublicKey,
        pk: minisign::PublicKey,
        sk: minisign::SecretKey,
    }

    impl Claves {
        pub fn nuevas() -> Self {
            let kp = minisign::KeyPair::generate_unencrypted_keypair().unwrap();
            let texto = kp.pk.to_box().unwrap().to_string();
            Self {
                publica: PublicKey::decode(&texto).unwrap(),
                pk: kp.pk,
                sk: kp.sk,
            }
        }

        pub fn firmar(&self, datos: &[u8]) -> Vec<u8> {
            minisign::sign(
                Some(&self.pk),
                &self.sk,
                std::io::Cursor::new(datos),
                None,
                None,
            )
            .unwrap()
            .to_string()
            .into_bytes()
        }
    }

    pub fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    /// Servidor HTTP local que sirve `ficheros`; devuelve la URL base con `/` final.
    pub async fn servidor_ficheros(ficheros: Vec<(&'static str, Vec<u8>)>) -> String {
        let mut r = Router::new();
        for (ruta, datos) in ficheros {
            r = r.route(ruta, get(move || async move { datos.clone() }));
        }
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
        format!("http://{addr}/")
    }

    /// URL de un puerto local cerrado (conexión rechazada).
    pub async fn url_cerrada() -> String {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = l.local_addr().unwrap();
        drop(l);
        format!("http://{addr}/")
    }

    #[derive(Default)]
    pub struct SystemdDoble {
        llamadas: Mutex<Vec<String>>,
    }

    impl SystemdDoble {
        pub fn llamadas(&self) -> Vec<String> {
            self.llamadas.lock().unwrap().clone()
        }
        fn apuntar(&self, s: String) -> Result<()> {
            self.llamadas.lock().unwrap().push(s);
            Ok(())
        }
    }

    impl Systemd for SystemdDoble {
        fn daemon_reload(&self) -> Result<()> {
            self.apuntar("daemon-reload".into())
        }
        fn enable_now(&self, u: &str) -> Result<()> {
            self.apuntar(format!("enable --now {u}"))
        }
        fn disable_now(&self, u: &str) -> Result<()> {
            self.apuntar(format!("disable --now {u}"))
        }
        fn restart(&self, u: &str) -> Result<()> {
            self.apuntar(format!("restart {u}"))
        }
        fn estado(&self, _: &str) -> EstadoUnidad {
            EstadoUnidad {
                activo: "active".into(),
                cpu_ns: Some(1_000),
                memoria: Some(4096),
            }
        }
        fn instalar_paquetes(&self, _: Familia, p: &[String]) -> Result<()> {
            if p.is_empty() {
                Ok(())
            } else {
                self.apuntar(format!("paquetes {}", p.join(" ")))
            }
        }
    }

    pub fn rutas_prueba(dir: &Path) -> Rutas {
        Rutas {
            modulos: dir.join("modules"),
            unidades: dir.join("systemd"),
            run: dir.join("run"),
            datos: dir.join("datos"),
        }
    }

    pub fn entorno(dir: &Path) -> (Entorno, Arc<SystemdDoble>) {
        let doble = Arc::new(SystemdDoble::default());
        let g = Gestor::new(
            rutas_prueba(dir),
            doble.clone(),
            "http://127.0.0.1:9/",
            None,
            None,
        );
        let mut env = g.entorno;
        env.espera_salud = Duration::from_millis(400);
        (env, doble)
    }

    pub fn gestor(
        dir: &Path,
        base_url: &str,
        clave: Option<PublicKey>,
    ) -> (Arc<Gestor>, Arc<SystemdDoble>) {
        let doble = Arc::new(SystemdDoble::default());
        let mut g = Gestor::new(
            rutas_prueba(dir),
            doble.clone(),
            base_url,
            clave,
            Some(Familia::Debian),
        );
        g.entorno.espera_salud = Duration::from_millis(400);
        (Arc::new(g), doble)
    }

    pub fn manifiesto(id: &str, version: &str, nucleo_minimo: &str) -> String {
        format!(
            "id = \"{id}\"\nnombre = \"Prueba\"\nversion = \"{version}\"\nnucleo_minimo = \"{nucleo_minimo}\"\n\n[[menu]]\ntitulo = \"Inicio\"\nruta = \"inicio\"\n"
        )
    }

    pub fn tar_modulo(id: &str, version: &str, nucleo_minimo: &str) -> Vec<u8> {
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut b = tar::Builder::new(gz);
        let mut poner = |ruta: &str, datos: &[u8], modo: u32| {
            let mut h = tar::Header::new_gnu();
            h.set_size(datos.len() as u64);
            h.set_mode(modo);
            h.set_cksum();
            b.append_data(&mut h, ruta, datos).unwrap();
        };
        poner(
            "manifest.toml",
            manifiesto(id, version, nucleo_minimo).as_bytes(),
            0o644,
        );
        poner(&format!("bin/netctl-mod-{id}"), b"#!/bin/sh\n", 0o755);
        poner(
            "ui/index.js",
            b"export function mount(){return()=>{}}",
            0o644,
        );
        b.into_inner().unwrap().finish().unwrap()
    }

    pub fn entrada(
        id: &str,
        version: &str,
        nucleo_minimo: &str,
        url: &str,
        sha: &str,
    ) -> catalogo::EntradaCatalogo {
        catalogo::EntradaCatalogo {
            id: id.into(),
            nombre: "Prueba".into(),
            descripcion: String::new(),
            version: version.parse().unwrap(),
            url: url.into(),
            sha256: sha.into(),
            nucleo_minimo: nucleo_minimo.parse().unwrap(),
        }
    }

    /// Módulo falso en `socket` con `/salud`, `/evento` y `/eco/{*r}` (devuelve ruta y cabeceras).
    pub async fn modulo_falso(socket: &Path) -> tokio::task::JoinHandle<()> {
        use axum::http::{HeaderMap, Uri};
        let r = Router::new()
            .route("/salud", get(|| async { "ok" }))
            .route("/evento", post(|| async { "ok" }))
            .route(
                "/eco/{*r}",
                axum::routing::any(|uri: Uri, h: HeaderMap, cuerpo: String| async move {
                    Json(json!({
                        "ruta": uri.to_string(),
                        "usuario": h.get("x-netctl-usuario").and_then(|v| v.to_str().ok()),
                        "cookie": h.contains_key("cookie"),
                        "csrf": h.contains_key("x-netctl-csrf"),
                        "cuerpo": cuerpo,
                    }))
                }),
            )
            .route(
                "/flujo",
                get(|| async {
                    let (tx, rx) = tokio::sync::mpsc::channel::<Result<String, std::io::Error>>(1);
                    tokio::spawn(async move {
                        tx.send(Ok("data: uno\n\n".into())).await.ok();
                        // No termina nunca: el cliente debe recibir «uno» sin esperar al final.
                        tokio::time::sleep(Duration::from_secs(3600)).await;
                        drop(tx);
                    });
                    (
                        [("content-type", "text/event-stream")],
                        axum::body::Body::from_stream(tokio_stream_de(rx)),
                    )
                }),
            );
        let s = socket.to_path_buf();
        let h = tokio::spawn(async move { netctl_sdk::servidor::servir_en(&s, r).await.unwrap() });
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        h
    }

    fn tokio_stream_de(
        mut rx: tokio::sync::mpsc::Receiver<Result<String, std::io::Error>>,
    ) -> impl futures_util::Stream<Item = Result<String, std::io::Error>> {
        futures_util::stream::poll_fn(move |cx| rx.poll_recv(cx))
    }
}

#[cfg(test)]
mod tests {
    use super::pruebas::*;
    use super::*;
    use crate::api::pruebas::*;
    use axum::http::StatusCode;

    #[tokio::test]
    async fn catalogo_inaccesible() {
        let d = tempfile::tempdir().unwrap();
        let base = url_cerrada().await;
        let (g, _) = gestor(d.path(), &base, Some(Claves::nuevas().publica));
        // Un módulo ya instalado.
        instalar::extraer(
            &tar_modulo("prueba", "0.1.0", "0.1.0"),
            &g.rutas().dir_modulo("prueba"),
        )
        .unwrap();
        assert!(matches!(g.catalogo().await, Err(ErrorCatalogo::Red(_))));
        let app = app_desde(estado(d.path()), super::rutas(g.clone()), "192.168.1.20");
        let s = entrar(&app).await;
        let (st, _, v) = pedir(&app, peticion("GET", "/api/catalogo", Some(&s), None)).await;
        assert_eq!(st, StatusCode::BAD_GATEWAY);
        assert!(
            v["error"]
                .as_str()
                .unwrap()
                .starts_with("No se pudo descargar el catálogo")
        );
        let (st, _, v) = pedir(&app, peticion("GET", "/api/modulos", Some(&s), None)).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["modulos"][0]["id"], "prueba");
        assert_eq!(v["modulos"][0]["activo"], "active");
        assert_eq!(v["modulos"][0]["memoria"], 4096);
        assert_eq!(v["modulos"][0]["menu"][0]["ruta"], "inicio");
    }

    #[tokio::test]
    async fn instalar_por_api_y_avisar() {
        let d = tempfile::tempdir().unwrap();
        let k = Claves::nuevas();
        let tar = tar_modulo("prueba", "0.1.0", "0.1.0");
        let sha = {
            use sha2::Digest;
            hex(&sha2::Sha256::digest(&tar))
        };
        let firma_tar = k.firmar(&tar);
        // El catálogo apunta al tar del mismo servidor: se sirve después de saber la URL.
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}/", l.local_addr().unwrap());
        let cat = Catalogo {
            modulos: vec![entrada(
                "prueba",
                "0.1.0",
                "0.1.0",
                &format!("{base}m.tar.gz"),
                &sha,
            )],
            nucleo: Some("9.0.0".parse().unwrap()),
        };
        let json = serde_json::to_vec(&cat).unwrap();
        let firma_cat = k.firmar(&json);
        let r = Router::new()
            .route(
                "/catalogo/index.json",
                get(move || async move { json.clone() }),
            )
            .route(
                "/catalogo/index.json.minisig",
                get(move || async move { firma_cat.clone() }),
            )
            .route("/m.tar.gz", get(move || async move { tar.clone() }))
            .route(
                "/m.tar.gz.minisig",
                get(move || async move { firma_tar.clone() }),
            );
        tokio::spawn(async move { axum::serve(l, r).await.unwrap() });
        let (g, doble) = gestor(d.path(), &base, Some(k.publica.clone()));
        let _m = modulo_falso(&g.rutas().socket("prueba")).await;
        let app = app_desde(estado(d.path()), super::rutas(g.clone()), "192.168.1.20");
        let s = entrar(&app).await;
        let (st, _, v) = pedir(&app, peticion("GET", "/api/catalogo", Some(&s), None)).await;
        assert_eq!(st, StatusCode::OK, "{v}");
        assert_eq!(v["nucleo_nuevo"], true);
        assert_eq!(v["modulos"][0]["instalado"], serde_json::Value::Null);
        let (st, _, v) = pedir(
            &app,
            peticion("POST", "/api/modulos/prueba/instalar", Some(&s), None),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{v}");
        assert!(
            doble
                .llamadas()
                .contains(&"enable --now netctl-mod-prueba.service".to_string())
        );
        let (st, _, _) = pedir(
            &app,
            peticion("POST", "/api/modulos/nada/instalar", Some(&s), None),
        )
        .await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    /// Prueba de los paquetes reales de `tools/`: instala un módulo publicado en un servidor HTTP
    /// local con la misma estructura que las Releases. Ver `docs/prueba-fase1.md`.
    /// `NETCTL_PRUEBA_URL=http://127.0.0.1:8000/ NETCTL_PRUEBA_CLAVE=clave.pub
    /// NETCTL_PRUEBA_MODULO=wireguard cargo test -p netctl-core -- --ignored paquete_real`
    #[tokio::test]
    #[ignore = "necesita un servidor local con paquetes de tools/empaquetar-modulo.sh"]
    async fn paquete_real_desde_servidor_local() {
        let var = |n: &str| std::env::var(n).unwrap_or_else(|_| panic!("falta {n}"));
        let id = var("NETCTL_PRUEBA_MODULO");
        let clave =
            catalogo::leer_clave(&std::fs::read_to_string(var("NETCTL_PRUEBA_CLAVE")).unwrap())
                .expect("clave pública no válida");
        let d = tempfile::tempdir().unwrap();
        let (g, doble) = gestor(d.path(), &var("NETCTL_PRUEBA_URL"), Some(clave));
        let cat = g.catalogo().await.expect("catálogo");
        assert!(cat.modulos.iter().any(|e| e.id == id), "{cat:?}");
        // systemd es un doble: un módulo falso responde a /salud en el socket.
        let _m = modulo_falso(&g.rutas().socket(&id)).await;
        g.instalar(&id).await.expect("instalar");
        let bin = g.rutas().binario(&id);
        let meta = std::fs::metadata(&bin).unwrap();
        use std::os::unix::fs::PermissionsExt;
        assert!(
            meta.permissions().mode() & 0o111 != 0,
            "binario sin permiso de ejecución"
        );
        assert_eq!(&std::fs::read(&bin).unwrap()[..4], b"\x7fELF");
        assert!(g.rutas().dir_modulo(&id).join("ui/index.js").is_file());
        assert!(g.rutas().unidad(&id).is_file());
        assert_eq!(g.instalados()[0].id, id);
        let llamadas = doble.llamadas();
        assert!(
            llamadas.contains(&format!("enable --now netctl-mod-{id}.service")),
            "{llamadas:?}"
        );
        eprintln!(
            "instalado {id} ({} bytes); systemd: {llamadas:?}",
            meta.len()
        );
    }

    /// Módulo falso que guarda los cuerpos de `/evento` y responde `/equipos` con `equipos`.
    async fn modulo_eventos(
        socket: &Path,
        equipos: serde_json::Value,
    ) -> (Arc<std::sync::Mutex<Vec<String>>>, tokio::task::JoinHandle<()>) {
        let vistos = Arc::new(std::sync::Mutex::new(Vec::new()));
        let v2 = vistos.clone();
        let r = Router::new()
            .route("/salud", get(|| async { "ok" }))
            .route(
                "/evento",
                post(move |cuerpo: String| {
                    let v = v2.clone();
                    async move {
                        v.lock().unwrap().push(cuerpo);
                        "ok"
                    }
                }),
            )
            .route("/equipos", get(move || async move { Json(equipos.clone()) }));
        let s = socket.to_path_buf();
        let h = tokio::spawn(async move { netctl_sdk::servidor::servir_en(&s, r).await.unwrap() });
        for _ in 0..100 {
            if socket.exists() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        (vistos, h)
    }

    #[tokio::test]
    async fn desinstalar_avisa_modulo() {
        let d = tempfile::tempdir().unwrap();
        let (g, _) = gestor(d.path(), "http://127.0.0.1:9/", None);
        instalar::extraer(&tar_modulo("firewall", "0.1.0", "0.1.0"), &g.rutas().dir_modulo("firewall")).unwrap();
        let (vistos, _h) = modulo_eventos(&g.rutas().socket("firewall"), json!([])).await;
        g.desinstalar("firewall", false, true).await.unwrap();
        let v = vistos.lock().unwrap().clone();
        assert!(!v.is_empty(), "el módulo no recibió el aviso");
        let e: serde_json::Value = serde_json::from_str(&v[0]).unwrap();
        assert_eq!(e["tipo"], "desinstalando");
        assert_eq!(e["restaurar"], true);
        assert!(!g.rutas().dir_modulo("firewall").exists());
    }

    #[tokio::test]
    async fn modulo_colgado_se_desinstala_por_su_binario() {
        use std::os::unix::fs::PermissionsExt;
        let d = tempfile::tempdir().unwrap();
        let (g, _) = gestor(d.path(), "http://127.0.0.1:9/", None);
        instalar::extraer(&tar_modulo("firewall", "0.1.0", "0.1.0"), &g.rutas().dir_modulo("firewall")).unwrap();
        let marca = d.path().join("llamado");
        let bin = g.rutas().binario("firewall");
        std::fs::write(&bin, format!("#!/bin/sh\necho \"$@\" > {}\n", marca.display())).unwrap();
        std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        // Sin socket: el aviso por HTTP falla y se usa el binario.
        g.desinstalar("firewall", false, true).await.unwrap();
        assert_eq!(std::fs::read_to_string(&marca).unwrap().trim(), "desinstalar --restaurar");
        assert!(!g.rutas().dir_modulo("firewall").exists());
    }

    #[tokio::test]
    async fn desinstalar_sin_modulo_vivo_sigue() {
        let d = tempfile::tempdir().unwrap();
        let (g, _) = gestor(d.path(), "http://127.0.0.1:9/", None);
        instalar::extraer(&tar_modulo("firewall", "0.1.0", "0.1.0"), &g.rutas().dir_modulo("firewall")).unwrap();
        g.desinstalar("firewall", false, false).await.unwrap();
        assert!(!g.rutas().dir_modulo("firewall").exists());
    }

    #[test]
    fn equipos_junta_y_deduplica() {
        let red = vec![
            json!({"nombre": "", "ip": "192.168.1.20", "mac": "aa:bb:cc:dd:ee:01", "red": "eth1", "conectado": false, "origen": "red"}),
            json!({"nombre": "", "ip": "192.168.1.30", "mac": "AA:BB:CC:DD:EE:02", "red": "eth1", "conectado": true, "origen": "red"}),
        ];
        let modulo = vec![
            json!({"nombre": "NAS", "ip": "192.168.1.20", "mac": "aa:bb:cc:dd:ee:01", "red": "eth1", "conectado": true, "origen": "red"}),
            json!({"nombre": "Móvil de Ana", "ip": "10.8.0.2", "mac": "", "red": "VPN", "conectado": true, "origen": "vpn", "id": "clave"}),
        ];
        let l = juntar_equipos(vec![red, modulo]);
        assert_eq!(l.len(), 3, "{l:?}");
        let nas = l.iter().find(|e| e["ip"] == "192.168.1.20").unwrap();
        assert_eq!(nas["nombre"], "NAS");
        assert_eq!(nas["conectado"], true);
        assert_eq!(l.iter().find(|e| e["ip"] == "192.168.1.30").unwrap()["mac"], "aa:bb:cc:dd:ee:02");
        // los conectados primero
        assert!(l.iter().take_while(|e| e["conectado"] == true).count() == 3);
    }

    #[test]
    fn parsea_vecinos_nucleo() {
        let j = r#"[{"dst":"192.168.1.20","dev":"eth1","lladdr":"aa:bb:cc:dd:ee:ff","state":["REACHABLE"]},{"dst":"192.168.1.22","dev":"eth1","state":["FAILED"]}]"#;
        let v = vecinos_de_json(j);
        assert_eq!(v.len(), 1);
        assert_eq!(v[0]["mac"], "aa:bb:cc:dd:ee:ff");
        assert_eq!(v[0]["conectado"], true);
    }

    #[tokio::test]
    async fn ruta_equipos_pregunta_a_modulos() {
        let d = tempfile::tempdir().unwrap();
        let (g, _) = gestor(d.path(), "http://127.0.0.1:9/", None);
        instalar::extraer(&tar_modulo("wireguard", "0.1.0", "0.1.0"), &g.rutas().dir_modulo("wireguard")).unwrap();
        let (_v, _h) = modulo_eventos(
            &g.rutas().socket("wireguard"),
            json!([{"nombre": "Móvil", "ip": "10.8.0.2", "mac": "", "red": "VPN", "conectado": true, "origen": "vpn"}]),
        )
        .await;
        let l = g.equipos(vec![]).await;
        assert_eq!(l.len(), 1);
        assert_eq!(l[0]["nombre"], "Móvil");
    }
}
