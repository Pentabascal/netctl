//! Órdenes del sistema y rutas, detrás de un trait y una estructura para probar sin root.

use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;

pub trait Sistema: Send + Sync {
    /// Ejecuta `programa args…` (con `entrada` por stdin si la hay) y devuelve su salida.
    fn ejecutar(&self, programa: &str, args: &[&str], entrada: Option<&str>) -> Result<String, String>;

    /// ¿Responde `ip:puerto` por TCP? (estado «no responde» de los reenvíos)
    fn conectar(&self, ip: std::net::IpAddr, puerto: u16) -> bool {
        std::net::TcpStream::connect_timeout(&(ip, puerto).into(), std::time::Duration::from_secs(2)).is_ok()
    }

    /// IPs de un nombre según el DNS del sistema (vacío si no resuelve).
    fn resolver(&self, nombre: &str) -> Vec<std::net::IpAddr> {
        use std::net::ToSocketAddrs;
        let mut v: Vec<std::net::IpAddr> = (nombre, 0).to_socket_addrs().map(|a| a.map(|x| x.ip()).collect()).unwrap_or_default();
        v.sort();
        v.dedup();
        v
    }

    /// Descarga `url` (https) en el fichero `destino`.
    fn descargar(&self, url: &str, destino: &Path) -> Result<(), String> {
        // Con tiempo máximo: una descarga colgada no puede dejar la base «descargando» siempre.
        let agente: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(std::time::Duration::from_secs(600))).build().into();
        let mut r = agente.get(url).call().map_err(|e| format!("No se pudo descargar {url}: {e}"))?;
        let mut f = std::fs::File::create(destino).map_err(|e| e.to_string())?;
        std::io::copy(&mut r.body_mut().as_reader(), &mut f).map_err(|e| format!("La descarga de {url} se cortó: {e}"))?;
        Ok(())
    }

    /// Instante UNIX actual (segundos).
    fn ahora(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

pub struct SistemaReal;

impl Sistema for SistemaReal {
    fn ejecutar(&self, programa: &str, args: &[&str], entrada: Option<&str>) -> Result<String, String> {
        let mut hijo = Command::new(programa)
            .args(args)
            .stdin(if entrada.is_some() { Stdio::piped() } else { Stdio::null() })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("No se pudo ejecutar {programa}: {e}"))?;
        if let Some(t) = entrada {
            let mut stdin = hijo.stdin.take().expect("stdin");
            stdin.write_all(t.as_bytes()).map_err(|e| format!("{programa}: {e}"))?;
        }
        let s = hijo.wait_with_output().map_err(|e| format!("{programa}: {e}"))?;
        if !s.status.success() {
            let err = String::from_utf8_lossy(&s.stderr);
            return Err(format!("{programa} {} falló: {}", args.join(" "), err.trim()));
        }
        Ok(String::from_utf8_lossy(&s.stdout).into_owned())
    }
}

/// Doble de pruebas: guarda cada orden y responde según el prefijo de la orden.
#[derive(Default)]
pub struct Falso {
    pub llamadas: Mutex<Vec<(String, Option<String>)>>,
    pub respuestas: Mutex<Vec<(String, Result<String, String>)>>,
    pub reloj: Mutex<u64>,
    /// Puertos (ip:puerto) que responden a `conectar`.
    pub abiertos: Mutex<Vec<String>>,
    /// Respuestas de `resolver`.
    pub dns: Mutex<std::collections::HashMap<String, Vec<std::net::IpAddr>>>,
    /// Ficheros que da `descargar` por URL.
    pub descargas: Mutex<std::collections::HashMap<String, Vec<u8>>>,
}

impl Falso {
    pub fn new() -> Self {
        Self::default()
    }
    /// Respuesta para las órdenes que empiezan por `prefijo` (la última añadida gana).
    pub fn responder(&self, prefijo: &str, r: Result<&str, &str>) {
        self.respuestas.lock().unwrap().push((prefijo.into(), r.map(Into::into).map_err(Into::into)));
    }
    pub fn ordenes(&self) -> Vec<String> {
        self.llamadas.lock().unwrap().iter().map(|(c, _)| c.clone()).collect()
    }
    /// Entradas (stdin) de las órdenes que empiezan por `prefijo`.
    pub fn entradas(&self, prefijo: &str) -> Vec<String> {
        self.llamadas
            .lock()
            .unwrap()
            .iter()
            .filter(|(c, _)| c.starts_with(prefijo))
            .filter_map(|(_, e)| e.clone())
            .collect()
    }
    pub fn responder_dns(&self, nombre: &str, ips: &[&str]) {
        self.dns.lock().unwrap().insert(nombre.into(), ips.iter().map(|i| i.parse().unwrap()).collect());
    }
    pub fn responder_descarga(&self, url: &str, datos: Vec<u8>) {
        self.descargas.lock().unwrap().insert(url.into(), datos);
    }
    pub fn limpiar(&self) {
        self.llamadas.lock().unwrap().clear();
    }
}

impl Sistema for Falso {
    fn ejecutar(&self, programa: &str, args: &[&str], entrada: Option<&str>) -> Result<String, String> {
        let orden = std::iter::once(programa).chain(args.iter().copied()).collect::<Vec<_>>().join(" ");
        self.llamadas.lock().unwrap().push((orden.clone(), entrada.map(Into::into)));
        let r = self.respuestas.lock().unwrap();
        r.iter().rev().find(|(p, _)| orden.starts_with(p.as_str())).map(|(_, r)| r.clone()).unwrap_or(Ok(String::new()))
    }
    fn ahora(&self) -> u64 {
        *self.reloj.lock().unwrap()
    }
    fn descargar(&self, url: &str, destino: &Path) -> Result<(), String> {
        let d = self.descargas.lock().unwrap().get(url).cloned().ok_or_else(|| format!("No se pudo descargar {url}: sin red"))?;
        std::fs::write(destino, d).map_err(|e| e.to_string())
    }
    fn resolver(&self, nombre: &str) -> Vec<std::net::IpAddr> {
        self.dns.lock().unwrap().get(nombre).cloned().unwrap_or_default()
    }
    fn conectar(&self, ip: std::net::IpAddr, puerto: u16) -> bool {
        self.abiertos.lock().unwrap().contains(&format!("{ip}:{puerto}"))
    }
}

/// Rutas del sistema que usa el módulo (cambiables en tests y en la demo).
#[derive(Clone, Debug)]
pub struct Rutas {
    /// `/var/lib/netctl/firewall`
    pub datos: PathBuf,
    /// `/etc/netctl` (core.toml)
    pub etc_netctl: PathBuf,
    /// `/etc/wireguard`
    pub wireguard: PathBuf,
    /// `/proc/net/route`
    pub proc_route: PathBuf,
    /// `/etc` (copia de /etc/firewalld y /etc/ufw al instalar)
    pub etc: PathBuf,
    /// Ayudante `netctl-dpi` (junto al binario del módulo).
    pub dpi: PathBuf,
}

impl Rutas {
    pub fn sistema() -> Self {
        Self {
            datos: "/var/lib/netctl/firewall".into(),
            etc_netctl: "/etc/netctl".into(),
            wireguard: "/etc/wireguard".into(),
            proc_route: "/proc/net/route".into(),
            etc: "/etc".into(),
            dpi: std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|d| d.join("netctl-dpi")))
                .unwrap_or_else(|| "netctl-dpi".into()),
        }
    }

    pub fn en(raiz: &Path) -> Self {
        Self {
            datos: raiz.join("var/lib/netctl/firewall"),
            etc_netctl: raiz.join("etc/netctl"),
            wireguard: raiz.join("etc/wireguard"),
            proc_route: raiz.join("proc/net/route"),
            etc: raiz.join("etc"),
            dpi: raiz.join("bin/netctl-dpi"),
        }
    }
}

/// Escribe `datos` en `ruta` de forma atómica con permisos 0600 (la carpeta se crea con 0700).
pub fn escribir_privado(ruta: &Path, datos: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = ruta.parent()
        && !dir.exists()
    {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut tmp = ruta.as_os_str().to_owned();
    tmp.push(".nuevo");
    let tmp = PathBuf::from(tmp);
    let _ = std::fs::remove_file(&tmp);
    {
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&tmp)?;
        f.write_all(datos)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, ruta)
}
