//! Llevar el proyecto al sistema: `/etc/wireguard/<iface>.conf`, `wg syncconf` o reinicio de
//! `wg-quick@<iface>`, y la tabla nftables `inet netctl_wg`.

use std::io::Write as _;
use std::process::{Command, Stdio};

use wg_modelo::{Proyecto, nft};

use crate::Rutas;

/// Órdenes del sistema, detrás de un trait para probar sin root.
pub trait Sistema: Send + Sync {
    /// Ejecuta `programa args…` (con `entrada` por stdin si la hay) y devuelve su salida.
    fn ejecutar(
        &self,
        programa: &str,
        args: &[&str],
        entrada: Option<&str>,
    ) -> Result<String, String>;

    /// Instante UNIX actual (en segundos).
    fn ahora(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0)
    }
}

pub struct SistemaReal;

impl Sistema for SistemaReal {
    fn ejecutar(
        &self,
        programa: &str,
        args: &[&str],
        entrada: Option<&str>,
    ) -> Result<String, String> {
        let mut hijo = Command::new(programa)
            .args(args)
            .stdin(if entrada.is_some() {
                Stdio::piped()
            } else {
                Stdio::null()
            })
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("No se pudo ejecutar {programa}: {e}"))?;
        if let Some(t) = entrada {
            let mut stdin = hijo.stdin.take().expect("stdin");
            stdin
                .write_all(t.as_bytes())
                .map_err(|e| format!("{programa}: {e}"))?;
        }
        let s = hijo
            .wait_with_output()
            .map_err(|e| format!("{programa}: {e}"))?;
        if !s.status.success() {
            let err = String::from_utf8_lossy(&s.stderr);
            return Err(format!(
                "{programa} {} falló: {}",
                args.join(" "),
                err.trim()
            ));
        }
        Ok(String::from_utf8_lossy(&s.stdout).into_owned())
    }
}

/// Qué ha cambiado: solo peers (no corta a nadie) o el propio servidor (reinicia la interfaz).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cambio {
    Peers,
    Servidor,
}

const SYSCTL: &str = "90-netctl-wireguard.conf";

fn unidad(iface: &str) -> String {
    format!("wg-quick@{}", iface.trim())
}

fn tiene_postup(conf: &str) -> bool {
    conf.lines()
        .map(str::trim_start)
        .any(|l| l.starts_with("PostUp") || l.starts_with("PostDown"))
}

/// Escribe la config y aplica el cambio. `firewall_presente`: el módulo Firewall pone el NAT.
pub fn aplicar(
    s: &dyn Sistema,
    r: &Rutas,
    p: &Proyecto,
    cambio: Cambio,
    firewall_presente: bool,
) -> Result<(), String> {
    // Todo lo que puede fallar por datos se calcula antes de tocar nada.
    let conf = p.config_servidor_panel()?;
    let reglas = nft::tabla_wg(p, !firewall_presente)?;
    let iface = p.servidor.interfaz.trim();
    let ruta = r.conf(iface)?;
    let anterior = std::fs::read_to_string(&ruta).ok();

    // Un .conf de wg-configurador quita sus reglas iptables en PostDown: hay que bajar la
    // interfaz con ese fichero todavía en su sitio.
    if cambio == Cambio::Servidor && anterior.as_deref().is_some_and(tiene_postup) {
        let _ = s.ejecutar("systemctl", &["stop", &unidad(iface)], None);
    }
    crate::escribir_privado(&ruta, conf.as_bytes())
        .map_err(|e| format!("No se pudo escribir {}: {e}", ruta.display()))?;
    reenvio(s, r)?;
    abrir_firewalld(s, iface, p.servidor.puerto.trim());

    match cambio {
        Cambio::Servidor => reiniciar(s, iface)?,
        Cambio::Peers => {
            let ruta_txt = ruta.to_string_lossy();
            let sync = s
                .ejecutar("wg-quick", &["strip", &ruta_txt], None)
                .and_then(|t| s.ejecutar("wg", &["syncconf", iface, "/dev/stdin"], Some(&t)));
            if let Err(e) = sync {
                // La interfaz no estaba levantada: arrancarla no corta a nadie.
                eprintln!("wg syncconf falló ({e}); se levanta la interfaz {iface}");
                reiniciar(s, iface)?;
            }
        }
    }
    s.ejecutar("nft", &["-f", "-"], Some(&reglas)).map(|_| ())
}

/// Si firewalld está activo (RHEL/SUSE por defecto), abre el puerto UDP de WireGuard y mete
/// la interfaz en la zona `trusted` para que deje reenviar su tráfico; los permisos por
/// cliente los sigue aplicando la tabla `netctl_wg`. Es best-effort: un fallo solo se registra.
fn abrir_firewalld(s: &dyn Sistema, iface: &str, puerto: &str) {
    if s.ejecutar("firewall-cmd", &["--state"], None).is_err() {
        return;
    }
    let puerto = format!("{puerto}/udp");
    for permanente in [true, false] {
        let mut base: Vec<&str> = Vec::new();
        if permanente {
            base.push("--permanent");
        }
        let mut a = base.clone();
        a.extend(["--add-port", puerto.as_str()]);
        let mut b = base.clone();
        b.extend(["--zone=trusted", "--add-interface", iface]);
        for args in [a, b] {
            if let Err(e) = s.ejecutar("firewall-cmd", &args, None) {
                eprintln!("firewall-cmd {} falló: {e}", args.join(" "));
            }
        }
    }
}

/// Solo recarga la tabla nftables (p. ej. al instalar o quitar el módulo Firewall).
pub fn solo_nft(s: &dyn Sistema, p: &Proyecto, firewall_presente: bool) -> Result<(), String> {
    let reglas = nft::tabla_wg(p, !firewall_presente)?;
    s.ejecutar("nft", &["-f", "-"], Some(&reglas)).map(|_| ())
}

/// Al arrancar el módulo: adopta una interfaz existente y recarga las reglas (nftables no
/// guarda nada entre reinicios). Solo reinicia la interfaz si hace falta:
/// - el `.conf` no existe → se crea y se levanta;
/// - el `.conf` es de wg-configurador (PostUp/PostDown con iptables) → avisa y reinicia una vez;
/// - el `.conf` es distinto → `syncconf`;
/// - igual → solo nftables.
pub fn adoptar(
    s: &dyn Sistema,
    r: &Rutas,
    p: &Proyecto,
    firewall_presente: bool,
) -> Result<(), String> {
    let iface = p.servidor.interfaz.trim();
    let nuevo = p.config_servidor_panel()?;
    match std::fs::read_to_string(r.conf(iface)?).ok() {
        None => aplicar(s, r, p, Cambio::Servidor, firewall_presente),
        Some(t) if tiene_postup(&t) => {
            eprintln!(
                "Aviso: /etc/wireguard/{iface}.conf tiene reglas PostUp/PostDown (wg-configurador). \
                 Se sustituyen por la tabla nftables de netctl y se reinicia la interfaz una vez."
            );
            aplicar(s, r, p, Cambio::Servidor, firewall_presente)
        }
        Some(t) if t != nuevo => aplicar(s, r, p, Cambio::Peers, firewall_presente),
        Some(_) => {
            reenvio(s, r)?;
            solo_nft(s, p, firewall_presente)
        }
    }
}

/// Deja de usar una interfaz (al renombrarla o reemplazar el proyecto).
pub fn retirar(s: &dyn Sistema, r: &Rutas, iface: &str) {
    // Un nombre no válido nunca llegó a usarse: no se toca nada.
    let Ok(ruta) = r.conf(iface) else { return };
    let _ = s.ejecutar("systemctl", &["disable", "--now", &unidad(iface)], None);
    let _ = std::fs::remove_file(ruta);
}

fn reiniciar(s: &dyn Sistema, iface: &str) -> Result<(), String> {
    let u = unidad(iface);
    s.ejecutar("systemctl", &["enable", &u], None)?;
    s.ejecutar("systemctl", &["restart", &u], None).map(|_| ())
}

/// Reenvío IPv4 activado ahora y en cada arranque (antes lo hacía el PostUp).
fn reenvio(s: &dyn Sistema, r: &Rutas) -> Result<(), String> {
    let f = r.sysctl.join(SYSCTL);
    if !f.exists() {
        std::fs::create_dir_all(&r.sysctl).map_err(|e| format!("{}: {e}", r.sysctl.display()))?;
        std::fs::write(
            &f,
            "# netctl: reenvío para el servidor WireGuard\nnet.ipv4.ip_forward = 1\n",
        )
        .map_err(|e| format!("No se pudo escribir {}: {e}", f.display()))?;
    }
    s.ejecutar("sysctl", &["-q", "-w", "net.ipv4.ip_forward=1"], None)
        .map(|_| ())
}
