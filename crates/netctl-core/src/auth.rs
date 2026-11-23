//! Administrador único: contraseña argon2id, TOTP, sesiones y bloqueo por intentos.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

pub const USUARIO: &str = "admin";
const MAX_FALLOS: u32 = 5;
const BLOQUEO_BASE: u64 = 60;
const BLOQUEO_MAX: u64 = 3600;
const DURACION_SESION: Duration = Duration::from_secs(12 * 3600);
const MAX_SESIONES: usize = 32;
const MAX_IPS: usize = 10_000;
const PASO_TOTP: u64 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorLogin {
    Credenciales,
    TotpRequerido,
    Bloqueado { segundos: u64 },
}

impl ErrorLogin {
    pub fn mensaje(&self) -> String {
        match self {
            Self::Credenciales => "Contraseña o código incorrectos".into(),
            Self::TotpRequerido => "Falta el código del doble factor".into(),
            Self::Bloqueado { segundos } => {
                format!("Demasiados intentos fallidos. Prueba de nuevo en {segundos} s")
            }
        }
    }
}

#[derive(Clone, Debug)]
pub struct Sesion {
    pub token: String,
    pub csrf: String,
    pub caduca: Instant,
    /// `generacion` de `admin.json` al crearla: si sube (cambio de contraseña o de TOTP,
    /// también desde otro proceso), la sesión deja de valer.
    pub generacion: u64,
}

impl Sesion {
    /// Compara el CSRF recibido en tiempo constante.
    pub fn csrf_valido(&self, recibido: &str) -> bool {
        bool::from(self.csrf.as_bytes().ct_eq(recibido.as_bytes()))
    }
}

/// Lo que se guarda en `admin.json` (0600).
#[derive(Serialize, Deserialize, Default)]
struct Datos {
    #[serde(default)]
    password: Option<String>,
    /// Secreto TOTP en base32; presente = doble factor activado.
    #[serde(default)]
    totp: Option<String>,
    /// Sube con cada cambio de credenciales; invalida las sesiones anteriores.
    #[serde(default)]
    generacion: u64,
}

#[derive(Default)]
struct Intentos {
    fallos: u32,
    bloqueos: u32,
    hasta: Option<Instant>,
}

#[derive(Default)]
struct Totp {
    pendiente: Option<Vec<u8>>,
    ultimo_paso: u64,
}

pub struct Almacen {
    ruta: PathBuf,
    datos: Mutex<Datos>,
    totp: Mutex<Totp>,
    sesiones: Mutex<HashMap<[u8; 32], Sesion>>,
    intentos: Mutex<HashMap<IpAddr, Intentos>>,
    /// Firma (mtime, tamaño, inodo) de `admin.json` la última vez que se leyó.
    firma: Mutex<Option<Firma>>,
}

type Firma = (Option<SystemTime>, u64, u64);

fn firma(ruta: &Path) -> Option<Firma> {
    use std::os::unix::fs::MetadataExt;
    let m = std::fs::metadata(ruta).ok()?;
    Some((m.modified().ok(), m.len(), m.ino()))
}

fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

fn aleatorio(n: usize) -> Vec<u8> {
    let mut b = vec![0u8; n];
    getrandom::getrandom(&mut b).expect("sin fuente de aleatoriedad");
    b
}

fn token_aleatorio() -> String {
    use base64::Engine;
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(aleatorio(32))
}

/// Clave del contador de intentos: la IPv4, o el /64 en IPv6 (un atacante suele tener el /64 entero).
fn clave_ip(ip: IpAddr) -> IpAddr {
    match ip.to_canonical() {
        IpAddr::V6(v6) => {
            let mut s = v6.segments();
            s[4..].fill(0);
            IpAddr::V6(s.into())
        }
        v4 => v4,
    }
}

fn unix_ahora() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn totp(secreto: Vec<u8>) -> Option<totp_rs::TOTP> {
    totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 1, PASO_TOTP, secreto).ok()
}

fn base32(b: &[u8]) -> String {
    match totp_rs::Secret::Raw(b.to_vec()).to_encoded() {
        totp_rs::Secret::Encoded(s) => s,
        totp_rs::Secret::Raw(_) => unreachable!(),
    }
}

fn hash_password(password: &str) -> Result<String> {
    use argon2::password_hash::{PasswordHasher, SaltString};
    let sal = SaltString::encode_b64(&aleatorio(16)).map_err(|e| anyhow::anyhow!("{e}"))?;
    argon2::Argon2::default()
        .hash_password(password.as_bytes(), &sal)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("No se pudo calcular el hash de la contraseña: {e}"))
}

fn verificar_password(hash: Option<&str>, password: &str) -> bool {
    use argon2::password_hash::{PasswordHash, PasswordVerifier};
    let Some(h) = hash.and_then(|h| PasswordHash::new(h).ok()) else {
        return false;
    };
    argon2::Argon2::default()
        .verify_password(password.as_bytes(), &h)
        .is_ok()
}

impl Almacen {
    /// Abre (o prepara, si no existe) `admin.json`.
    pub fn abrir(ruta: &Path) -> Result<Self> {
        let datos = match std::fs::read_to_string(ruta) {
            Ok(t) => serde_json::from_str(&t)
                .with_context(|| format!("{} no es válido", ruta.display()))?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Datos::default(),
            Err(e) => return Err(e).with_context(|| format!("No se pudo leer {}", ruta.display())),
        };
        Ok(Self {
            ruta: ruta.to_path_buf(),
            datos: Mutex::new(datos),
            totp: Mutex::default(),
            sesiones: Mutex::default(),
            intentos: Mutex::default(),
            firma: Mutex::new(firma(ruta)),
        })
    }

    /// Vuelve a leer `admin.json` (lo puede cambiar `netctl-core admin ...` con el servicio en marcha).
    fn recargar(&self) {
        let f = firma(&self.ruta);
        if let Ok(d) = std::fs::read_to_string(&self.ruta)
            .map_err(anyhow::Error::from)
            .and_then(|t| Ok(serde_json::from_str::<Datos>(&t)?))
        {
            *self.datos.lock().unwrap() = d;
            *self.firma.lock().unwrap() = f;
        }
    }

    /// Recarga `admin.json` solo si ha cambiado en disco (se mira en cada petición).
    fn recargar_si_cambio(&self) {
        let f = firma(&self.ruta);
        if f.is_some() && *self.firma.lock().unwrap() != f {
            self.recargar();
        }
    }

    fn guardar(&self, d: &Datos) -> Result<()> {
        if let Some(dir) = self.ruta.parent() {
            crate::ficheros::crear_dir(dir, 0o700)?;
        }
        crate::ficheros::escribir_atomico(
            &self.ruta,
            serde_json::to_string_pretty(d)?.as_bytes(),
            0o600,
        )
    }

    pub fn hay_password(&self) -> bool {
        self.datos.lock().unwrap().password.is_some()
    }

    /// Cambia la contraseña y cierra todas las sesiones.
    pub fn fijar_password(&self, password: &str) -> Result<()> {
        if password.is_empty() {
            anyhow::bail!("La contraseña no puede estar vacía");
        }
        let hash = hash_password(password)?;
        self.recargar();
        let mut d = self.datos.lock().unwrap();
        let anterior = d.password.replace(hash);
        d.generacion += 1;
        if let Err(e) = self.guardar(&d) {
            d.password = anterior;
            d.generacion -= 1;
            return Err(e);
        }
        self.sesiones.lock().unwrap().clear();
        Ok(())
    }

    pub fn comprobar_password(&self, password: &str) -> bool {
        let hash = self.datos.lock().unwrap().password.clone();
        verificar_password(hash.as_deref(), password)
    }

    /// Quita el doble factor (solo desde la línea de órdenes, como root).
    pub fn quitar_totp(&self) -> Result<()> {
        self.recargar();
        let mut d = self.datos.lock().unwrap();
        d.totp = None;
        d.generacion += 1;
        self.guardar(&d)?;
        drop(d);
        self.sesiones.lock().unwrap().clear();
        Ok(())
    }

    pub fn login(
        &self,
        password: &str,
        totp: Option<&str>,
        ip: IpAddr,
    ) -> Result<Sesion, ErrorLogin> {
        self.login_en(password, totp, ip, Instant::now())
    }

    /// Como `login` pero con el instante dado (para probar el bloqueo).
    pub fn login_en(
        &self,
        password: &str,
        totp: Option<&str>,
        ip: IpAddr,
        ahora: Instant,
    ) -> Result<Sesion, ErrorLogin> {
        let ip = clave_ip(ip);
        self.recargar();
        if let Some(hasta) = self.intentos.lock().unwrap().get(&ip).and_then(|i| i.hasta)
            && hasta > ahora
        {
            return Err(ErrorLogin::Bloqueado {
                segundos: (hasta - ahora).as_secs_f64().ceil() as u64,
            });
        }
        let (hash, secreto, generacion) = {
            let d = self.datos.lock().unwrap();
            (d.password.clone(), d.totp.clone(), d.generacion)
        };
        // Sin código no se mira la contraseña: así no se revela si era correcta.
        if secreto.is_some() && totp.is_none() {
            return Err(ErrorLogin::TotpRequerido);
        }
        let ok = verificar_password(hash.as_deref(), password)
            && match (&secreto, totp) {
                (Some(s), Some(c)) => self.comprobar_codigo(s, c, unix_ahora()),
                _ => true,
            };
        if !ok {
            return Err(self.fallo(ip, ahora));
        }
        self.intentos.lock().unwrap().remove(&ip);
        let sesion = Sesion {
            token: token_aleatorio(),
            csrf: token_aleatorio(),
            caduca: ahora + DURACION_SESION,
            generacion,
        };
        let mut sesiones = self.sesiones.lock().unwrap();
        sesiones.retain(|_, s| s.caduca > ahora);
        if sesiones.len() >= MAX_SESIONES
            && let Some(k) = sesiones
                .iter()
                .min_by_key(|(_, s)| s.caduca)
                .map(|(k, _)| *k)
        {
            sesiones.remove(&k);
        }
        sesiones.insert(hash_token(&sesion.token), sesion.clone());
        Ok(sesion)
    }

    fn fallo(&self, ip: IpAddr, ahora: Instant) -> ErrorLogin {
        let mut intentos = self.intentos.lock().unwrap();
        if intentos.len() >= MAX_IPS {
            intentos.retain(|_, i| i.hasta.is_some_and(|h| h > ahora) || i.bloqueos > 0);
            if intentos.len() >= MAX_IPS {
                intentos.clear();
            }
        }
        let i = intentos.entry(ip).or_default();
        i.fallos += 1;
        if i.fallos < MAX_FALLOS {
            return ErrorLogin::Credenciales;
        }
        i.fallos = 0;
        i.bloqueos += 1;
        let segundos = BLOQUEO_BASE
            .saturating_mul(1 << (i.bloqueos - 1).min(16))
            .min(BLOQUEO_MAX);
        i.hasta = Some(ahora + Duration::from_secs(segundos));
        ErrorLogin::Bloqueado { segundos }
    }

    /// Comprueba un código TOTP (±1 paso) sin aceptar dos veces el mismo paso.
    fn comprobar_codigo(&self, secreto_b32: &str, codigo: &str, unix: u64) -> bool {
        let Some(t) = totp_rs::Secret::Encoded(secreto_b32.to_string())
            .to_bytes()
            .ok()
            .and_then(totp)
        else {
            return false;
        };
        self.comprobar_con(&t, codigo, unix)
    }

    fn comprobar_con(&self, t: &totp_rs::TOTP, codigo: &str, unix: u64) -> bool {
        let mut estado = self.totp.lock().unwrap();
        let paso = unix / PASO_TOTP;
        for p in [paso.saturating_sub(1), paso, paso + 1] {
            let esperado = t.generate(p * PASO_TOTP);
            if bool::from(esperado.as_bytes().ct_eq(codigo.as_bytes())) && p > estado.ultimo_paso {
                estado.ultimo_paso = p;
                return true;
            }
        }
        false
    }

    /// Sesión viva con ese token.
    pub fn sesion(&self, token: &str) -> Option<Sesion> {
        // `netctl-core admin ...` puede haber cambiado la contraseña desde otro proceso.
        self.recargar_si_cambio();
        let generacion = self.datos.lock().unwrap().generacion;
        let k = hash_token(token);
        let mut sesiones = self.sesiones.lock().unwrap();
        match sesiones.get(&k) {
            Some(s) if s.caduca > Instant::now() && s.generacion >= generacion => Some(s.clone()),
            Some(_) => {
                sesiones.remove(&k);
                None
            }
            None => None,
        }
    }

    pub fn cerrar(&self, token: &str) {
        self.sesiones.lock().unwrap().remove(&hash_token(token));
    }

    pub fn totp_activo(&self) -> bool {
        self.datos.lock().unwrap().totp.is_some()
    }

    /// Genera un secreto nuevo pendiente de confirmar. Devuelve `(secreto_base32, uri_otpauth)`.
    pub fn activar_totp(&self) -> Result<(String, String)> {
        let secreto = aleatorio(20);
        let b32 = base32(&secreto);
        self.totp.lock().unwrap().pendiente = Some(secreto);
        let uri = format!(
            "otpauth://totp/netctl:{USUARIO}?secret={b32}&issuer=netctl&algorithm=SHA1&digits=6&period={PASO_TOTP}"
        );
        Ok((b32, uri))
    }

    /// Confirma el secreto pendiente con un código válido; desde ese momento el TOTP es obligatorio.
    pub fn confirmar_totp(&self, codigo: &str) -> bool {
        let Some(secreto) = self.totp.lock().unwrap().pendiente.clone() else {
            return false;
        };
        let Some(t) = totp(secreto.clone()) else {
            return false;
        };
        if !self.comprobar_con(&t, codigo, unix_ahora()) {
            return false;
        }
        self.recargar();
        let mut d = self.datos.lock().unwrap();
        let anterior = d.totp.replace(base32(&secreto));
        if self.guardar(&d).is_err() {
            d.totp = anterior;
            return false;
        }
        self.totp.lock().unwrap().pendiente = None;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn almacen() -> (tempfile::TempDir, Almacen) {
        let dir = tempfile::tempdir().unwrap();
        let a = Almacen::abrir(&dir.path().join("admin.json")).unwrap();
        a.fijar_password("correcta").unwrap();
        (dir, a)
    }

    fn codigo(secreto: &str, desfase: i64) -> String {
        let s = totp_rs::Secret::Encoded(secreto.into()).to_bytes().unwrap();
        let t = totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 1, 30, s).unwrap();
        let ahora = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        t.generate((ahora + desfase) as u64)
    }

    #[test]
    fn password_y_sesion() {
        let (dir, a) = almacen();
        let ip: IpAddr = "192.168.1.2".parse().unwrap();
        assert_eq!(
            a.login("mala", None, ip).unwrap_err(),
            ErrorLogin::Credenciales
        );
        let s = a.login("correcta", None, ip).unwrap();
        assert!(a.sesion(&s.token).is_some());
        assert!(a.sesion("otro").is_none());
        assert!(a.sesion(&s.token).unwrap().csrf_valido(&s.csrf));
        assert!(!a.sesion(&s.token).unwrap().csrf_valido("x"));
        a.cerrar(&s.token);
        assert!(a.sesion(&s.token).is_none());
        // Persistencia y permisos.
        use std::os::unix::fs::PermissionsExt;
        let ruta = dir.path().join("admin.json");
        assert_eq!(
            std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let texto = std::fs::read_to_string(&ruta).unwrap();
        assert!(texto.contains("$argon2id$") && !texto.contains("correcta"));
        let b = Almacen::abrir(&ruta).unwrap();
        assert!(b.comprobar_password("correcta"));
    }

    #[test]
    fn cambiar_password_desde_otro_proceso_cierra_sesiones() {
        // `a` es el servicio; `b`, la orden `netctl-core admin ...` (otro proceso, mismo fichero).
        let (dir, a) = almacen();
        let ruta = dir.path().join("admin.json");
        let ip: IpAddr = "192.168.1.2".parse().unwrap();
        let s = a.login("correcta", None, ip).unwrap();
        assert!(a.sesion(&s.token).is_some());

        let b = Almacen::abrir(&ruta).unwrap();
        b.fijar_password("nueva-larga").unwrap();
        assert!(a.sesion(&s.token).is_none(), "la sesión anterior debe morir");
        assert!(a.login("correcta", None, ip).is_err());

        // Las sesiones creadas después del cambio siguen valiendo.
        let s2 = a.login("nueva-larga", None, ip).unwrap();
        assert!(a.sesion(&s2.token).is_some());
        assert!(a.sesion(&s2.token).is_some());

        // Quitar el TOTP desde la línea de órdenes también las cierra.
        let c = Almacen::abrir(&ruta).unwrap();
        c.quitar_totp().unwrap();
        assert!(a.sesion(&s2.token).is_none());

        // El contador queda en admin.json.
        let v: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&ruta).unwrap()).unwrap();
        assert!(v["generacion"].as_u64().unwrap() >= 3, "{v}");
    }

    #[test]
    fn bloqueo_tras_5() {
        let (_d, a) = almacen();
        let ip: IpAddr = "203.0.113.9".parse().unwrap();
        let t0 = Instant::now();
        for _ in 0..4 {
            assert_eq!(
                a.login_en("mala", None, ip, t0).unwrap_err(),
                ErrorLogin::Credenciales
            );
        }
        assert_eq!(
            a.login_en("mala", None, ip, t0).unwrap_err(),
            ErrorLogin::Bloqueado { segundos: 60 }
        );
        // Ni con la buena mientras dure.
        assert!(matches!(
            a.login_en("correcta", None, ip, t0).unwrap_err(),
            ErrorLogin::Bloqueado { .. }
        ));
        // Otra IP no está afectada.
        assert!(
            a.login_en("correcta", None, "203.0.113.10".parse().unwrap(), t0)
                .is_ok()
        );
        // Pasado el bloqueo, otros 5 fallos bloquean el doble.
        let t1 = t0 + Duration::from_secs(61);
        for _ in 0..4 {
            assert_eq!(
                a.login_en("mala", None, ip, t1).unwrap_err(),
                ErrorLogin::Credenciales
            );
        }
        assert_eq!(
            a.login_en("mala", None, ip, t1).unwrap_err(),
            ErrorLogin::Bloqueado { segundos: 120 }
        );
        // Un acierto limpia el historial.
        let t2 = t1 + Duration::from_secs(121);
        assert!(a.login_en("correcta", None, ip, t2).is_ok());
        for _ in 0..4 {
            assert_eq!(
                a.login_en("mala", None, ip, t2).unwrap_err(),
                ErrorLogin::Credenciales
            );
        }
        assert_eq!(
            a.login_en("mala", None, ip, t2).unwrap_err(),
            ErrorLogin::Bloqueado { segundos: 60 }
        );
    }

    #[test]
    fn totp_obligatorio_si_activo() {
        let (_d, a) = almacen();
        let ip: IpAddr = "192.168.1.2".parse().unwrap();
        assert!(!a.totp_activo());
        let (secreto, uri) = a.activar_totp().unwrap();
        assert!(uri.starts_with("otpauth://totp/") && uri.contains(&secreto));
        // Pendiente: aún no se exige.
        assert!(!a.totp_activo());
        assert!(a.login("correcta", None, ip).is_ok());
        assert!(!a.confirmar_totp("abc"));
        assert!(a.confirmar_totp(&codigo(&secreto, 0)));
        assert!(a.totp_activo());
        assert_eq!(
            a.login("correcta", None, ip).unwrap_err(),
            ErrorLogin::TotpRequerido
        );
        assert_eq!(
            a.login("correcta", Some("123"), ip).unwrap_err(),
            ErrorLogin::Credenciales
        );
        assert_eq!(
            a.login("mala", Some(&codigo(&secreto, 30)), ip)
                .unwrap_err(),
            ErrorLogin::Credenciales
        );
        let bueno = codigo(&secreto, 30);
        assert!(a.login("correcta", Some(&bueno), ip).is_ok());
        // El mismo código no vale dos veces.
        assert_eq!(
            a.login("correcta", Some(&bueno), ip).unwrap_err(),
            ErrorLogin::Credenciales
        );
    }
}
