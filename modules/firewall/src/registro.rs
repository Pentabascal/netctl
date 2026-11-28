//! Registro de cambios (`registro.jsonl`), el más nuevo primero al listar.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

pub const MAX_ENTRADAS: usize = 1000;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoCambio {
    Aplicado,
    Pendiente,
    Confirmado,
    Revertido,
    Error,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entrada {
    pub fecha: u64,
    pub usuario: String,
    pub resumen: String,
    pub estado: EstadoCambio,
    #[serde(default)]
    pub detalle: Option<String>,
}

pub struct Registro {
    ruta: PathBuf,
    cerrojo: Mutex<()>,
}

impl Registro {
    pub fn new(dir: &Path) -> Self {
        Self { ruta: dir.join("registro.jsonl"), cerrojo: Mutex::new(()) }
    }
    /// Añade una entrada. Al pasar de `MAX_ENTRADAS + 10 %`, se queda con las `MAX_ENTRADAS`
    /// más nuevas. Los fallos de escritura solo se avisan en el log del servicio.
    pub fn anotar(&self, e: Entrada) {
        use std::io::Write as _;
        use std::os::unix::fs::OpenOptionsExt;
        let _g = self.cerrojo.lock().unwrap();
        if let Some(dir) = self.ruta.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let Ok(linea) = serde_json::to_string(&e) else { return };
        let r = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.ruta)
            .and_then(|mut f| writeln!(f, "{linea}"));
        if let Err(err) = r {
            eprintln!("No se pudo escribir el registro: {err}");
            return;
        }
        let todas = self.leer();
        if todas.len() > MAX_ENTRADAS + MAX_ENTRADAS / 10 {
            let resto = &todas[todas.len() - MAX_ENTRADAS..];
            let texto: String = resto.iter().filter_map(|x| serde_json::to_string(x).ok()).map(|l| l + "\n").collect();
            let _ = crate::sistema::escribir_privado(&self.ruta, texto.as_bytes());
        }
    }

    fn leer(&self) -> Vec<Entrada> {
        std::fs::read_to_string(&self.ruta)
            .map(|t| t.lines().filter_map(|l| serde_json::from_str(l).ok()).collect())
            .unwrap_or_default()
    }

    /// Las `n` entradas más nuevas, la más nueva primero.
    pub fn listar(&self, n: usize) -> Vec<Entrada> {
        let _g = self.cerrojo.lock().unwrap();
        let mut v = self.leer();
        v.reverse();
        v.truncate(n);
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn e(n: u64) -> Entrada {
        Entrada { fecha: n, usuario: "admin".into(), resumen: format!("cambio {n}"), estado: EstadoCambio::Aplicado, detalle: None }
    }

    #[test]
    fn anota_y_lista_al_reves() {
        let d = tempfile::tempdir().unwrap();
        let r = Registro::new(d.path());
        assert!(r.listar(10).is_empty());
        for i in 0..3 {
            r.anotar(e(i));
        }
        let l = r.listar(2);
        assert_eq!(l.iter().map(|x| x.fecha).collect::<Vec<_>>(), vec![2, 1]);
    }

    #[test]
    fn rota() {
        let d = tempfile::tempdir().unwrap();
        let r = Registro::new(d.path());
        for i in 0..(MAX_ENTRADAS as u64 + 50) {
            r.anotar(e(i));
        }
        let l = r.listar(usize::MAX);
        assert!(l.len() <= MAX_ENTRADAS + MAX_ENTRADAS / 10, "{}", l.len());
        assert_eq!(l[0].fecha, MAX_ENTRADAS as u64 + 49);
    }
}
