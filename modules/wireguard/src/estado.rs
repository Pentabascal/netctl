//! Estado persistente: `/var/lib/netctl/wireguard/proyecto.json` (0600).

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use wg_modelo::Proyecto;

/// Proyecto guardado con un número de revisión que sube en cada cambio. Una petición que trae
/// una revisión vieja (otra pestaña cambió algo entre medias) se rechaza con 409.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Documento {
    pub revision: u64,
    pub proyecto: Proyecto,
}

pub struct Almacen {
    ruta: PathBuf,
}

impl Almacen {
    pub fn new(dir: &Path) -> Self {
        Self {
            ruta: dir.join("proyecto.json"),
        }
    }

    pub fn ruta(&self) -> &Path {
        &self.ruta
    }

    /// `None` si todavía no hay servidor configurado.
    pub fn cargar(&self) -> Result<Option<Documento>, String> {
        match std::fs::read_to_string(&self.ruta) {
            Ok(t) => serde_json::from_str(&t)
                .map(Some)
                .map_err(|e| format!("{} no es válido: {e}", self.ruta.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("No se pudo leer {}: {e}", self.ruta.display())),
        }
    }

    pub fn guardar(&self, d: &Documento) -> Result<(), String> {
        let json = serde_json::to_vec_pretty(d).map_err(|e| e.to_string())?;
        crate::escribir_privado(&self.ruta, &json)
            .map_err(|e| format!("No se pudo guardar {}: {e}", self.ruta.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ida_y_vuelta() {
        let dir = tempfile::tempdir().unwrap();
        let a = Almacen::new(&dir.path().join("wireguard"));
        assert!(a.cargar().unwrap().is_none());
        let mut p = Proyecto::nuevo();
        p.anadir_cliente();
        a.guardar(&Documento {
            revision: 3,
            proyecto: p,
        })
        .unwrap();
        let d = a.cargar().unwrap().unwrap();
        assert_eq!(d.revision, 3);
        assert_eq!(d.proyecto.clientes.len(), 1);
    }
}
