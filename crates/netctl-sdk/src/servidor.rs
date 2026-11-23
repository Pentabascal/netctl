//! Servidor HTTP del módulo sobre un socket Unix.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::net::UnixListener;
use axum::Router;
use axum::http::HeaderMap;

/// Ruta del socket: `NETCTL_SOCKET` si está definida, si no `/run/netctl/mod-<id>.sock`.
pub fn ruta_socket(id: &str) -> PathBuf {
    ruta_socket_de(id, std::env::var_os("NETCTL_SOCKET").map(PathBuf::from))
}

fn ruta_socket_de(id: &str, env: Option<PathBuf>) -> PathBuf {
    env.filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| PathBuf::from(format!("/run/netctl/mod-{id}.sock")))
}

/// Sirve `router` en el socket del módulo `id` (0600, borra uno anterior).
pub async fn servir(id: &str, router: Router) -> Result<()> {
    servir_en(&ruta_socket(id), router).await
}

pub async fn servir_en(ruta: &Path, router: Router) -> Result<()> {
    if let Some(dir) = ruta.parent() {
        std::fs::create_dir_all(dir).with_context(|| format!("No se pudo crear {}", dir.display()))?;
    }
    // Se crea con otro nombre, se le ponen permisos 0600 y se renombra: así nunca hay un
    // instante en que el socket definitivo sea accesible para otros usuarios.
    let mut tmp = ruta.as_os_str().to_owned();
    tmp.push(".nuevo");
    let tmp = PathBuf::from(tmp);
    borrar_si_existe(&tmp)?;
    let escucha = UnixListener::bind(&tmp).with_context(|| format!("No se pudo abrir {}", tmp.display()))?;
    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
    std::fs::rename(&tmp, ruta).with_context(|| format!("No se pudo crear {}", ruta.display()))?;
    axum::serve(escucha, router).await?;
    Ok(())
}

fn borrar_si_existe(ruta: &Path) -> Result<()> {
    match std::fs::remove_file(ruta) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => {
            Err(e).with_context(|| format!("No se pudo borrar {}", ruta.display()))
        }
        _ => Ok(()),
    }
}

/// Usuario del panel que hace la petición (cabecera `X-Netctl-Usuario` que pone el núcleo).
pub fn usuario(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-netctl-usuario")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(String::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::routing::get;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[test]
    fn ruta_por_defecto_y_env() {
        assert_eq!(ruta_socket_de("wireguard", None), PathBuf::from("/run/netctl/mod-wireguard.sock"));
        assert_eq!(ruta_socket_de("wireguard", Some("/tmp/x.sock".into())), PathBuf::from("/tmp/x.sock"));
    }

    #[test]
    fn lee_usuario() {
        let mut h = HeaderMap::new();
        assert_eq!(usuario(&h), None);
        h.insert("x-netctl-usuario", "admin".parse().unwrap());
        assert_eq!(usuario(&h).as_deref(), Some("admin"));
    }

    #[tokio::test]
    async fn sirve_en_socket_600() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("sub/mod-prueba.sock");
        std::fs::create_dir_all(ruta.parent().unwrap()).unwrap();
        std::fs::write(&ruta, b"viejo").unwrap(); // un socket anterior se borra
        let app = Router::new().route("/salud", get(|| async { "ok" }));
        let r = ruta.clone();
        tokio::spawn(async move { servir_en(&r, app).await.unwrap() });
        let mut s = None;
        for _ in 0..100 {
            if let Ok(c) = tokio::net::UnixStream::connect(&ruta).await {
                s = Some(c);
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let mut s = s.expect("no se pudo conectar al socket");
        let modo = std::fs::metadata(&ruta).unwrap().permissions().mode() & 0o777;
        assert_eq!(modo, 0o600);
        s.write_all(b"GET /salud HTTP/1.1\r\nHost: m\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut resp = String::new();
        s.read_to_string(&mut resp).await.unwrap();
        assert!(resp.starts_with("HTTP/1.1 200"), "{resp}");
        assert!(resp.ends_with("ok"), "{resp}");
    }
}
