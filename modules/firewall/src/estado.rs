//! Estado persistente: la última configuración confirmada y la pendiente de confirmar.

use std::path::{Path, PathBuf};

use crate::modelo::Config;
use crate::sistema::escribir_privado;

pub struct Almacen {
    dir: PathBuf,
}

impl Almacen {
    pub fn new(dir: &Path) -> Self {
        Self { dir: dir.to_path_buf() }
    }
    pub fn dir(&self) -> &Path {
        &self.dir
    }
    fn cargar(&self, nombre: &str) -> Result<Option<Config>, String> {
        let ruta = self.dir.join(nombre);
        match std::fs::read_to_string(&ruta) {
            Ok(t) => serde_json::from_str(&t).map(Some).map_err(|e| format!("{} no es válido: {e}", ruta.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("No se pudo leer {}: {e}", ruta.display())),
        }
    }
    fn guardar(&self, nombre: &str, c: &Config) -> Result<(), String> {
        let ruta = self.dir.join(nombre);
        let t = serde_json::to_vec_pretty(c).map_err(|e| e.to_string())?;
        escribir_privado(&ruta, &t).map_err(|e| format!("No se pudo guardar {}: {e}", ruta.display()))
    }
    pub fn cargar_confirmada(&self) -> Result<Option<Config>, String> {
        self.cargar("confirmada.json")
    }
    pub fn guardar_confirmada(&self, c: &Config) -> Result<(), String> {
        self.guardar("confirmada.json", c)
    }
    pub fn cargar_pendiente(&self) -> Result<Option<Config>, String> {
        self.cargar("pendiente.json")
    }
    pub fn guardar_pendiente(&self, c: &Config) -> Result<(), String> {
        self.guardar("pendiente.json", c)
    }
    pub fn borrar_pendiente(&self) {
        let _ = std::fs::remove_file(self.dir.join("pendiente.json"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::*;
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn guarda_y_carga() {
        let d = tempfile::tempdir().unwrap();
        let a = Almacen::new(&d.path().join("fw"));
        assert!(a.cargar_confirmada().unwrap().is_none());
        a.guardar_confirmada(&router()).unwrap();
        assert_eq!(a.cargar_confirmada().unwrap().unwrap(), router());
        let modo = std::fs::metadata(d.path().join("fw/confirmada.json")).unwrap().permissions().mode();
        assert_eq!(modo & 0o777, 0o600);
        a.guardar_pendiente(&router()).unwrap();
        assert!(a.cargar_pendiente().unwrap().is_some());
        a.borrar_pendiente();
        assert!(a.cargar_pendiente().unwrap().is_none());
        a.borrar_pendiente();
    }

    #[test]
    fn json_roto_da_error() {
        let d = tempfile::tempdir().unwrap();
        std::fs::write(d.path().join("confirmada.json"), "{").unwrap();
        assert!(Almacen::new(d.path()).cargar_confirmada().is_err());
    }
}
