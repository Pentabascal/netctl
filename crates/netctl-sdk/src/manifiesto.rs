//! `manifest.toml` de un módulo.

use anyhow::{Context, Result, bail};
use semver::Version;
use serde::Deserialize;

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct EntradaMenu {
    pub titulo: String,
    pub ruta: String,
    #[serde(default)]
    pub icono: String,
}

#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Paquetes {
    pub debian: Vec<String>,
    pub suse: Vec<String>,
    pub rhel: Vec<String>,
}

#[derive(Deserialize, Clone, Debug, PartialEq)]
pub struct Manifiesto {
    pub id: String,
    pub nombre: String,
    pub version: Version,
    pub nucleo_minimo: Version,
    #[serde(default)]
    pub icono: String,
    #[serde(default)]
    pub menu: Vec<EntradaMenu>,
    #[serde(default)]
    pub paquetes: Paquetes,
    #[serde(default)]
    pub conflictos: Vec<String>,
}

impl Manifiesto {
    pub fn leer(toml: &str) -> Result<Self> {
        let m: Self = toml::from_str(toml).context("manifest.toml no válido")?;
        let id_ok = !m.id.is_empty()
            && m.id.len() <= 32
            && m.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        if !id_ok {
            bail!("Id de módulo «{}» no válido: solo minúsculas, cifras y guiones", m.id);
        }
        Ok(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EJEMPLO: &str = r#"
id = "wireguard"
nombre = "WireGuard"
version = "0.1.0"
nucleo_minimo = "0.1.0"
icono = "escudo"
conflictos = ["wg-quick@wg0"]

[[menu]]
titulo = "Clientes"
ruta = "clientes"
icono = "usuarios"

[[menu]]
titulo = "Servidor"
ruta = "servidor"

[paquetes]
debian = ["wireguard-tools", "nftables"]
suse = ["wireguard-tools", "nftables"]
rhel = ["wireguard-tools", "nftables"]
"#;

    #[test]
    fn manifiesto_ejemplo() {
        let m = Manifiesto::leer(EJEMPLO).unwrap();
        assert_eq!(m.id, "wireguard");
        assert_eq!(m.version, Version::new(0, 1, 0));
        assert_eq!(m.nucleo_minimo, Version::new(0, 1, 0));
        assert_eq!(m.menu.len(), 2);
        assert_eq!(m.menu[0].titulo, "Clientes");
        assert_eq!(m.menu[1].icono, "");
        assert_eq!(m.paquetes.suse, ["wireguard-tools", "nftables"]);
        assert_eq!(m.conflictos, ["wg-quick@wg0"]);
    }

    #[test]
    fn manifiesto_id_invalido() {
        let malo = EJEMPLO.replace(r#"id = "wireguard""#, r#"id = "../etc""#);
        assert!(Manifiesto::leer(&malo).is_err());
        let sin_version = EJEMPLO.replace(r#"version = "0.1.0""#, "");
        assert!(Manifiesto::leer(&sin_version).is_err());
    }
}
