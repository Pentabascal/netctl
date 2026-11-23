//! Módulo WireGuard de netctl: servidor, clientes con permisos por cliente (nftables) y estado
//! en vivo. Escucha HTTP en el socket del módulo; el núcleo le hace de proxy autenticado.
//!
//! Todo lo que toca el sistema (wg, wg-quick, systemctl, nft, sysctl) pasa por
//! [`aplicar::Sistema`] y todas las rutas por [`Rutas`], para poder probarlo sin root.

pub mod api;
pub mod aplicar;
pub mod estado;
pub mod sugerencias;
pub mod vivo;

use std::io::Write as _;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

/// Rutas del sistema que usa el módulo (cambiables en tests y en la demo).
#[derive(Clone, Debug)]
pub struct Rutas {
    /// `/var/lib/netctl/wireguard` (estado: `proyecto.json`)
    pub datos: PathBuf,
    /// `/etc/wireguard`
    pub wireguard: PathBuf,
    /// `/etc/sysctl.d`
    pub sysctl: PathBuf,
    /// `/usr/lib/netctl/modules` (para saber si está instalado el módulo `firewall`)
    pub modulos: PathBuf,
    /// `/proc/net/route` (sugerencias del asistente)
    pub proc_route: PathBuf,
    /// `/sys/class/net` (interfaces de red)
    pub sys_net: PathBuf,
}

impl Rutas {
    pub fn sistema() -> Self {
        Self {
            datos: "/var/lib/netctl/wireguard".into(),
            wireguard: "/etc/wireguard".into(),
            sysctl: "/etc/sysctl.d".into(),
            modulos: "/usr/lib/netctl/modules".into(),
            proc_route: "/proc/net/route".into(),
            sys_net: "/sys/class/net".into(),
        }
    }

    /// Todas las rutas dentro de `raiz` (tests y demo).
    pub fn en(raiz: &Path) -> Self {
        Self {
            datos: raiz.join("var/lib/netctl/wireguard"),
            wireguard: raiz.join("etc/wireguard"),
            sysctl: raiz.join("etc/sysctl.d"),
            modulos: raiz.join("usr/lib/netctl/modules"),
            proc_route: raiz.join("proc/net/route"),
            sys_net: raiz.join("sys/class/net"),
        }
    }

    /// `/etc/wireguard/<interfaz>.conf`. Se niega con nombres que no son de interfaz
    /// (`..`, `/`…): con ellos se podría escribir o borrar un `.conf` cualquiera como root.
    pub fn conf(&self, interfaz: &str) -> Result<PathBuf, String> {
        let i = wg_modelo::validar_interfaz("Interfaz", interfaz)?;
        Ok(self.wireguard.join(format!("{i}.conf")))
    }
}

/// Escribe `datos` en `ruta` de forma atómica con permisos 0600 (la carpeta se crea con 0700).
pub fn escribir_privado(ruta: &Path, datos: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = ruta.parent()
        && !dir.exists()
    {
        std::fs::create_dir_all(dir)?;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let mut tmp = ruta.as_os_str().to_owned();
    tmp.push(".nuevo");
    let tmp = PathBuf::from(tmp);
    let _ = std::fs::remove_file(&tmp);
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(datos)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, ruta)
}
