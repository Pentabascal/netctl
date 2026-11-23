//! `/etc/netctl/core.toml`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

pub const RUTA_CONFIG: &str = "/etc/netctl/core.toml";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Alcance {
    /// Solo redes privadas (RFC1918, ULA, enlace local) y las de las interfaces WireGuard.
    #[default]
    SoloLan,
    /// Cualquier origen. Exige doble factor.
    Internet,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Config {
    pub puerto: u16,
    pub alcance: Alcance,
    pub datos: PathBuf,
    /// Carpeta con `cert.pem` y `key.pem`.
    pub tls: PathBuf,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            puerto: 8443,
            alcance: Alcance::SoloLan,
            datos: PathBuf::from("/var/lib/netctl"),
            tls: PathBuf::from(crate::tls::DIR_TLS),
        }
    }
}

impl Config {
    /// Lee la configuración; si el fichero no existe, devuelve la de por defecto.
    pub fn cargar(ruta: &Path) -> Result<Self> {
        match std::fs::read_to_string(ruta) {
            Ok(t) => toml::from_str(&t).with_context(|| format!("{} no es válido", ruta.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e).with_context(|| format!("No se pudo leer {}", ruta.display())),
        }
    }

    pub fn guardar(&self, ruta: &Path) -> Result<()> {
        if let Some(dir) = ruta.parent() {
            std::fs::create_dir_all(dir)
                .with_context(|| format!("No se pudo crear {}", dir.display()))?;
        }
        crate::ficheros::escribir_atomico(ruta, toml::to_string(self)?.as_bytes(), 0o644)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn por_defecto_y_vuelta() {
        let dir = tempfile::tempdir().unwrap();
        let ruta = dir.path().join("netctl/core.toml");
        let c = Config::cargar(&ruta).unwrap();
        assert_eq!(c, Config::default());
        assert_eq!(c.puerto, 8443);
        let c2 = Config {
            alcance: Alcance::Internet,
            ..c
        };
        c2.guardar(&ruta).unwrap();
        assert_eq!(Config::cargar(&ruta).unwrap(), c2);
        assert!(
            std::fs::read_to_string(&ruta)
                .unwrap()
                .contains("alcance = \"internet\"")
        );
    }
}
