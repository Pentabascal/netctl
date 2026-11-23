//! Catálogo firmado (`index.json` + `.minisig` en la Release `catalogo`).

use minisign_verify::{PublicKey, Signature};
use semver::Version;
use serde::{Deserialize, Serialize};

pub const URL_BASE: &str = "https://github.com/Pentabascal/netctl/releases/download/";
const LIMITE_CATALOGO: usize = 1024 * 1024;
const LIMITE_FIRMA: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct EntradaCatalogo {
    pub id: String,
    pub nombre: String,
    #[serde(default)]
    pub descripcion: String,
    pub version: Version,
    pub url: String,
    pub sha256: String,
    pub nucleo_minimo: Version,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Catalogo {
    pub modulos: Vec<EntradaCatalogo>,
    /// Última versión publicada del núcleo (para avisar de que hay una nueva).
    #[serde(default)]
    pub nucleo: Option<Version>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ErrorCatalogo {
    /// No se pudo descargar (sin red, GitHub caído, 404…).
    Red(String),
    Firma,
    Formato(String),
    /// El núcleo se compiló sin clave pública.
    SinClave,
}

impl std::fmt::Display for ErrorCatalogo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Red(e) => write!(f, "No se pudo descargar el catálogo de módulos: {e}"),
            Self::Firma => write!(f, "El catálogo de módulos no tiene una firma válida"),
            Self::Formato(e) => write!(f, "El catálogo de módulos no es válido: {e}"),
            Self::SinClave => write!(
                f,
                "Este núcleo no tiene configurada la clave pública de firma"
            ),
        }
    }
}

impl std::error::Error for ErrorCatalogo {}

/// Clave pública compilada en el núcleo (`clave_publica.txt`). `None` si aún es el marcador.
pub fn clave_publica() -> Option<PublicKey> {
    PublicKey::decode(include_str!("../../clave_publica.txt").trim()).ok()
}

/// Lee una clave en formato fichero minisign (dos líneas) o solo la base64.
pub fn leer_clave(texto: &str) -> Option<PublicKey> {
    let t = texto.trim();
    PublicKey::decode(t)
        .or_else(|_| PublicKey::from_base64(t))
        .ok()
}

/// ¿`firma` (texto `.minisig`) es válida para `datos` con `clave`?
pub fn verificar(clave: &PublicKey, datos: &[u8], firma: &[u8]) -> bool {
    let Ok(texto) = std::str::from_utf8(firma) else {
        return false;
    };
    let Ok(sig) = Signature::decode(texto) else {
        return false;
    };
    clave.verify(datos, &sig, false).is_ok()
}

/// Cliente HTTP del núcleo (rustls con `ring`; instala el proveedor si hace falta).
pub fn cliente_http() -> reqwest::Client {
    crate::tls::instalar_proveedor();
    reqwest::Client::builder()
        .user_agent(concat!("netctl/", env!("CARGO_PKG_VERSION")))
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .expect("cliente HTTP")
}

/// Descarga `url` con un tope de `limite` bytes.
pub async fn descargar(
    cliente: &reqwest::Client,
    url: &str,
    limite: usize,
) -> Result<Vec<u8>, String> {
    let mut r = cliente
        .get(url)
        .send()
        .await
        .map_err(|e| e.without_url().to_string())?;
    if !r.status().is_success() {
        return Err(format!("el servidor respondió {}", r.status()));
    }
    if r.content_length().is_some_and(|l| l > limite as u64) {
        return Err("el fichero es demasiado grande".into());
    }
    let mut datos = Vec::new();
    while let Some(trozo) = r.chunk().await.map_err(|e| e.without_url().to_string())? {
        if datos.len() + trozo.len() > limite {
            return Err("el fichero es demasiado grande".into());
        }
        datos.extend_from_slice(&trozo);
    }
    Ok(datos)
}

/// Descarga `<base_url>catalogo/index.json` y su firma, y la verifica.
pub async fn obtener(
    cliente: &reqwest::Client,
    base_url: &str,
    clave: &PublicKey,
) -> Result<Catalogo, ErrorCatalogo> {
    let url = format!("{base_url}catalogo/index.json");
    let datos = descargar(cliente, &url, LIMITE_CATALOGO)
        .await
        .map_err(ErrorCatalogo::Red)?;
    let firma = descargar(cliente, &format!("{url}.minisig"), LIMITE_FIRMA)
        .await
        .map_err(ErrorCatalogo::Red)?;
    if !verificar(clave, &datos, &firma) {
        return Err(ErrorCatalogo::Firma);
    }
    serde_json::from_slice(&datos).map_err(|e| ErrorCatalogo::Formato(e.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modulos::pruebas::*;

    fn catalogo_ejemplo() -> Catalogo {
        Catalogo {
            modulos: vec![EntradaCatalogo {
                id: "wireguard".into(),
                nombre: "WireGuard".into(),
                descripcion: "VPN".into(),
                version: Version::new(0, 1, 0),
                url: "http://x/netctl-mod-wireguard-0.1.0-x86_64.tar.gz".into(),
                sha256: "00".into(),
                nucleo_minimo: Version::new(0, 1, 0),
            }],
            nucleo: Some(Version::new(0, 2, 0)),
        }
    }

    #[tokio::test]
    async fn catalogo_firmado_se_acepta() {
        let k = Claves::nuevas();
        let json = serde_json::to_vec(&catalogo_ejemplo()).unwrap();
        let base = servidor_ficheros(vec![
            ("/catalogo/index.json", json.clone()),
            ("/catalogo/index.json.minisig", k.firmar(&json)),
        ])
        .await;
        let c = obtener(&cliente_http(), &base, &k.publica).await.unwrap();
        assert_eq!(c, catalogo_ejemplo());
    }

    #[tokio::test]
    async fn catalogo_firma_mala_rechaza() {
        let k = Claves::nuevas();
        let otra = Claves::nuevas();
        let json = serde_json::to_vec(&catalogo_ejemplo()).unwrap();
        let base = servidor_ficheros(vec![
            ("/catalogo/index.json", json.clone()),
            ("/catalogo/index.json.minisig", otra.firmar(&json)),
        ])
        .await;
        assert_eq!(
            obtener(&cliente_http(), &base, &k.publica).await,
            Err(ErrorCatalogo::Firma)
        );
        // Firmado bien pero con el contenido cambiado.
        let mut cambiado = json.clone();
        cambiado.extend_from_slice(b" ");
        let base = servidor_ficheros(vec![
            ("/catalogo/index.json", cambiado),
            ("/catalogo/index.json.minisig", k.firmar(&json)),
        ])
        .await;
        assert_eq!(
            obtener(&cliente_http(), &base, &k.publica).await,
            Err(ErrorCatalogo::Firma)
        );
    }

    #[tokio::test]
    async fn catalogo_sin_red() {
        let k = Claves::nuevas();
        let base = url_cerrada().await;
        assert!(matches!(
            obtener(&cliente_http(), &base, &k.publica).await,
            Err(ErrorCatalogo::Red(_))
        ));
    }
}
