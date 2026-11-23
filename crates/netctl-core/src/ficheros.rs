//! Escritura de ficheros con permisos fijos desde el primer byte.

use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::{Context, Result};

/// Escribe en un temporal con `modo` y lo renombra sobre `ruta`.
pub fn escribir_atomico(ruta: &Path, datos: &[u8], modo: u32) -> Result<()> {
    let mut tmp = ruta.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = std::path::PathBuf::from(tmp);
    let _ = std::fs::remove_file(&tmp);
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(modo)
        .open(&tmp)
        .with_context(|| format!("No se pudo crear {}", tmp.display()))?;
    f.write_all(datos)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, ruta).with_context(|| format!("No se pudo escribir {}", ruta.display()))
}

/// Crea `dir` (y padres) y le pone permisos `modo`.
pub fn crear_dir(dir: &Path, modo: u32) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).with_context(|| format!("No se pudo crear {}", dir.display()))?;
    std::fs::set_permissions(dir, std::fs::Permissions::from_mode(modo))?;
    Ok(())
}
