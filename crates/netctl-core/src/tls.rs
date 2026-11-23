//! Certificado del panel: autofirmado en el primer arranque, sustituible por uno propio.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

pub const DIR_TLS: &str = "/etc/netctl/tls";

/// Devuelve `(cert.pem, key.pem)` de `dir`; si falta alguno, genera un par autofirmado
/// (clave 0600) para `nombres` (más `localhost`).
pub fn asegurar_certificado(dir: &Path, nombres: &[String]) -> Result<(PathBuf, PathBuf)> {
    let cert = dir.join("cert.pem");
    let clave = dir.join("key.pem");
    if cert.exists() && clave.exists() {
        return Ok((cert, clave));
    }
    crate::ficheros::crear_dir(dir, 0o700)?;
    let mut sans = vec!["localhost".to_string()];
    sans.extend(nombres.iter().filter(|n| !n.is_empty()).cloned());
    sans.dedup();
    let par =
        rcgen::generate_simple_self_signed(sans).context("No se pudo generar el certificado")?;
    crate::ficheros::escribir_atomico(&clave, par.key_pair.serialize_pem().as_bytes(), 0o600)?;
    crate::ficheros::escribir_atomico(&cert, par.cert.pem().as_bytes(), 0o644)?;
    Ok((cert, clave))
}

/// Instala `ring` como proveedor de rustls para todo el proceso (idempotente).
pub fn instalar_proveedor() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn genera_una_vez() {
        let dir = tempfile::tempdir().unwrap();
        let tls = dir.path().join("tls");
        let (cert, clave) = asegurar_certificado(&tls, &["panel.lan".into()]).unwrap();
        let pem = std::fs::read_to_string(&cert).unwrap();
        assert!(pem.starts_with("-----BEGIN CERTIFICATE-----"));
        assert!(
            std::fs::read_to_string(&clave)
                .unwrap()
                .contains("PRIVATE KEY")
        );
        assert_eq!(
            std::fs::metadata(&clave).unwrap().permissions().mode() & 0o777,
            0o600
        );
        // Segunda vez: no se toca.
        asegurar_certificado(&tls, &[]).unwrap();
        assert_eq!(std::fs::read_to_string(&cert).unwrap(), pem);
    }
}
