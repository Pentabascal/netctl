//! Detección de la familia de distro por `/etc/os-release` e instalación de paquetes.

use anyhow::{Context, Result, bail};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Familia {
    Debian,
    Suse,
    Rhel,
}

fn campo<'a>(os_release: &'a str, clave: &str) -> &'a str {
    os_release
        .lines()
        .filter_map(|l| l.trim().split_once('='))
        .find(|(k, _)| k.trim() == clave)
        .map(|(_, v)| v.trim().trim_matches(|c| c == '"' || c == '\''))
        .unwrap_or("")
}

fn familia_de(id: &str) -> Option<Familia> {
    match id {
        "debian" | "ubuntu" => Some(Familia::Debian),
        "suse" | "opensuse" | "sles" | "sled" | "opensuse-leap" | "opensuse-tumbleweed" => Some(Familia::Suse),
        "rhel" | "centos" | "fedora" | "rocky" | "almalinux" => Some(Familia::Rhel),
        _ => None,
    }
}

/// Familia a partir del contenido de `os-release`: primero `ID`, luego cada palabra de `ID_LIKE`.
pub fn detectar_de(os_release: &str) -> Option<Familia> {
    std::iter::once(campo(os_release, "ID"))
        .chain(campo(os_release, "ID_LIKE").split_whitespace())
        .find_map(|id| familia_de(&id.to_ascii_lowercase()))
}

pub fn detectar() -> Option<Familia> {
    ["/etc/os-release", "/usr/lib/os-release"]
        .iter()
        .find_map(|r| std::fs::read_to_string(r).ok())
        .and_then(|t| detectar_de(&t))
}

/// Programa, argumentos y variables de entorno para instalar `paquetes` en la familia `f`.
pub fn comando_instalar(f: Familia, paquetes: &[String]) -> (&'static str, Vec<String>, Vec<(&'static str, &'static str)>) {
    let p = paquetes.iter().cloned();
    match f {
        Familia::Debian => (
            "apt-get",
            ["install", "-y"].map(String::from).into_iter().chain(p).collect(),
            vec![("DEBIAN_FRONTEND", "noninteractive")],
        ),
        Familia::Suse => {
            ("zypper", ["--non-interactive", "install"].map(String::from).into_iter().chain(p).collect(), vec![])
        }
        Familia::Rhel => ("dnf", ["install", "-y"].map(String::from).into_iter().chain(p).collect(), vec![]),
    }
}

pub fn instalar_paquetes(f: Familia, paquetes: &[String]) -> Result<()> {
    if paquetes.is_empty() {
        return Ok(());
    }
    let (prog, args, env) = comando_instalar(f, paquetes);
    let salida = std::process::Command::new(prog)
        .args(&args)
        .envs(env)
        .output()
        .with_context(|| format!("No se pudo ejecutar {prog}"))?;
    if !salida.status.success() {
        bail!(
            "No se pudieron instalar los paquetes {}: {}",
            paquetes.join(", "),
            String::from_utf8_lossy(&salida.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detecta_familias() {
        let debian = "PRETTY_NAME=\"Debian GNU/Linux 12 (bookworm)\"\nNAME=\"Debian GNU/Linux\"\nVERSION_ID=\"12\"\nID=debian\n";
        let ubuntu = "NAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nID=ubuntu\nID_LIKE=debian\n";
        let leap = "NAME=\"openSUSE Leap\"\nVERSION=\"15.6\"\nID=\"opensuse-leap\"\nID_LIKE=\"suse opensuse\"\n";
        let sles = "NAME=\"SLES\"\nVERSION_ID=\"15.6\"\nID=\"sles\"\nID_LIKE=\"suse\"\n";
        let rocky = "NAME=\"Rocky Linux\"\nVERSION_ID=\"9.4\"\nID=\"rocky\"\nID_LIKE=\"rhel centos fedora\"\n";
        let fedora = "NAME=\"Fedora Linux\"\nVERSION_ID=41\nID=fedora\n";
        let arch = "NAME=\"Arch Linux\"\nID=arch\nBUILD_ID=rolling\n";
        assert_eq!(detectar_de(debian), Some(Familia::Debian));
        assert_eq!(detectar_de(ubuntu), Some(Familia::Debian));
        assert_eq!(detectar_de(leap), Some(Familia::Suse));
        assert_eq!(detectar_de(sles), Some(Familia::Suse));
        assert_eq!(detectar_de(rocky), Some(Familia::Rhel));
        assert_eq!(detectar_de(fedora), Some(Familia::Rhel));
        assert_eq!(detectar_de(arch), None);
        assert_eq!(detectar_de(""), None);
    }

    #[test]
    fn comandos_por_familia() {
        let p = vec!["wireguard-tools".to_string(), "nftables".to_string()];
        let (prog, args, env) = comando_instalar(Familia::Debian, &p);
        assert_eq!(prog, "apt-get");
        assert_eq!(args, ["install", "-y", "wireguard-tools", "nftables"]);
        assert!(env.contains(&("DEBIAN_FRONTEND", "noninteractive")));
        let (prog, args, _) = comando_instalar(Familia::Suse, &p);
        assert_eq!(prog, "zypper");
        assert_eq!(args, ["--non-interactive", "install", "wireguard-tools", "nftables"]);
        let (prog, args, _) = comando_instalar(Familia::Rhel, &p);
        assert_eq!(prog, "dnf");
        assert_eq!(args, ["install", "-y", "wireguard-tools", "nftables"]);
    }

    #[test]
    fn sin_paquetes_no_hace_nada() {
        assert!(instalar_paquetes(Familia::Debian, &[]).is_ok());
    }
}
