//! Lo que el firewall lee del sistema: interfaces, vecinos, ruta por defecto y lo que hay que
//! dejar siempre abierto (panel, SSH, WireGuard).

use std::net::{IpAddr, Ipv4Addr};

use serde::Serialize;

use crate::modelo::{Config, Fijas, Iface};
use crate::sistema::{Rutas, Sistema};

/// Equipo visto en la red (tabla de vecinos ARP/NDP).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Vecino {
    pub ip: IpAddr,
    pub mac: String,
    pub iface: String,
    pub conectado: bool,
}

/// `ip -j addr` → interfaces.
pub fn parse_ip_addr(json: &str) -> Vec<Iface> {
    let Ok(v) = serde_json::from_str::<Vec<serde_json::Value>>(json) else { return vec![] };
    v.iter()
        .filter_map(|i| {
            let nombre = i.get("ifname")?.as_str()?.to_string();
            let mac = i
                .get("address")
                .and_then(|a| a.as_str())
                .filter(|a| crate::validar::es_mac(a) && *a != "00:00:00:00:00:00")
                .map(str::to_lowercase);
            let estado = i.get("operstate").and_then(|o| o.as_str()).unwrap_or("");
            let up = i
                .get("flags")
                .and_then(|f| f.as_array())
                .is_some_and(|f| f.iter().any(|x| x.as_str() == Some("UP")));
            let activa = estado == "UP" || (estado == "UNKNOWN" && up);
            let ips = i
                .get("addr_info")
                .and_then(|a| a.as_array())
                .map(|a| {
                    a.iter()
                        .filter_map(|x| {
                            let ip: IpAddr = x.get("local")?.as_str()?.parse().ok()?;
                            let p = x.get("prefixlen")?.as_u64()? as u8;
                            ipnet::IpNet::new(ip, p).ok()
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(Iface { nombre, ips, mac, activa })
        })
        .collect()
}

/// `ip -j neigh` → vecinos con MAC.
pub fn parse_neigh(json: &str) -> Vec<Vecino> {
    let Ok(v) = serde_json::from_str::<Vec<serde_json::Value>>(json) else { return vec![] };
    v.iter()
        .filter_map(|n| {
            let ip: IpAddr = n.get("dst")?.as_str()?.parse().ok()?;
            let mac = n.get("lladdr")?.as_str()?.to_lowercase();
            if !crate::validar::es_mac(&mac) {
                return None;
            }
            let iface = n.get("dev").and_then(|d| d.as_str()).unwrap_or("").to_string();
            let estados: Vec<&str> = n
                .get("state")
                .and_then(|s| s.as_array())
                .map(|a| a.iter().filter_map(|x| x.as_str()).collect())
                .unwrap_or_default();
            let conectado = estados.iter().any(|e| matches!(*e, "REACHABLE" | "DELAY" | "PROBE" | "PERMANENT"));
            Some(Vecino { ip, mac, iface, conectado })
        })
        .collect()
}

/// Interfaz de la ruta por defecto en `/proc/net/route`.
pub fn parse_ruta_defecto(texto: &str) -> Option<String> {
    texto.lines().skip(1).find_map(|l| {
        let c: Vec<&str> = l.split_whitespace().collect();
        (c.len() >= 2 && c[1] == "00000000").then(|| c[0].to_string())
    })
}

/// `(puerto, alcance_internet)` de `core.toml`; por defecto (8443, false).
pub fn parse_core_toml(texto: &str) -> (u16, bool) {
    let Ok(v) = texto.parse::<toml::Table>() else { return (8443, false) };
    let puerto = v
        .get("puerto")
        .and_then(|p| p.as_integer())
        .and_then(|p| u16::try_from(p).ok())
        .filter(|p| *p > 0)
        .unwrap_or(8443);
    let internet = v.get("alcance").and_then(|a| a.as_str()) == Some("internet");
    (puerto, internet)
}

/// `ListenPort` de un `.conf` de WireGuard.
pub fn parse_listen_port(conf: &str) -> Option<u16> {
    conf.lines().find_map(|l| {
        let (k, v) = l.split_once('=')?;
        if k.trim().eq_ignore_ascii_case("listenport") { v.trim().parse().ok() } else { None }
    })
}

/// Primer `port N` de `sshd -T`; 22 si no se sabe.
pub fn parse_sshd(salida: &str) -> u16 {
    salida
        .lines()
        .find_map(|l| l.strip_prefix("port ").and_then(|p| p.trim().parse().ok()))
        .unwrap_or(22)
}

/// Contadores de `nft -j list table inet netctl`, sumados por comentario corto: `f7`, `r5`
/// (reenvíos y reglas: hasta el primer `_`) o el comentario entero (`p_externa_gateway`…).
pub fn parse_contadores(json: &str) -> std::collections::BTreeMap<String, (u64, u64)> {
    let mut m = std::collections::BTreeMap::new();
    let Ok(v) = serde_json::from_str::<serde_json::Value>(json) else { return m };
    for item in v.get("nftables").and_then(|x| x.as_array()).into_iter().flatten() {
        let Some(regla) = item.get("rule") else { continue };
        let Some(com) = regla.get("comment").and_then(|c| c.as_str()) else { continue };
        let clave = match com.split_once('_') {
            Some((p, _))
                if p.len() > 1
                    && (p.starts_with('f') || p.starts_with('r'))
                    && p[1..].chars().all(|c| c.is_ascii_digit()) =>
            {
                p.to_string()
            }
            _ => com.to_string(),
        };
        for e in regla.get("expr").and_then(|x| x.as_array()).into_iter().flatten() {
            if let Some(c) = e.get("counter") {
                let p = c.get("packets").and_then(|x| x.as_u64()).unwrap_or(0);
                let b = c.get("bytes").and_then(|x| x.as_u64()).unwrap_or(0);
                let x = m.entry(clave.clone()).or_insert((0, 0));
                x.0 += p;
                x.1 += b;
            }
        }
    }
    m
}

pub fn interfaces(s: &dyn Sistema) -> Vec<Iface> {
    s.ejecutar("ip", &["-j", "addr"], None).map(|j| parse_ip_addr(&j)).unwrap_or_default()
}

pub fn vecinos(s: &dyn Sistema) -> Vec<Vecino> {
    s.ejecutar("ip", &["-j", "neigh"], None).map(|j| parse_neigh(&j)).unwrap_or_default()
}

pub fn ruta_defecto(r: &Rutas) -> Option<String> {
    std::fs::read_to_string(&r.proc_route).ok().and_then(|t| parse_ruta_defecto(&t))
}

/// Reglas fijas a partir del sistema y de la configuración (IPs de la zona Externa).
pub fn fijas(s: &dyn Sistema, r: &Rutas, c: &Config) -> Fijas {
    let (panel, alcance_internet) = std::fs::read_to_string(r.etc_netctl.join("core.toml"))
        .map(|t| parse_core_toml(&t))
        .unwrap_or((8443, false));
    let ssh = s.ejecutar("sshd", &["-T"], None).map(|o| parse_sshd(&o)).unwrap_or(22);
    let mut wg = vec![];
    if let Ok(dir) = std::fs::read_dir(&r.wireguard) {
        for e in dir.flatten() {
            let ruta = e.path();
            if ruta.extension().and_then(|x| x.to_str()) != Some("conf") {
                continue;
            }
            let Some(nombre) = ruta.file_stem().and_then(|x| x.to_str()) else { continue };
            if !crate::validar::es_iface(nombre) {
                continue;
            }
            if let Some(p) = std::fs::read_to_string(&ruta).ok().and_then(|t| parse_listen_port(&t)) {
                wg.push((nombre.to_string(), p));
            }
        }
    }
    wg.sort();
    Fijas { panel, alcance_internet, ssh, wg, ips_externas: ips_externas(c) }
}

/// IPv4 de las interfaces de la zona Externa.
pub fn ips_externas(c: &Config) -> Vec<Ipv4Addr> {
    c.ifaces_externas()
        .iter()
        .flat_map(|i| c.direcciones.get(i).cloned().unwrap_or_default())
        .filter_map(|n| match n.addr() {
            IpAddr::V4(v) => Some(v),
            IpAddr::V6(_) => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::router;
    use crate::sistema::Falso;

    const IP_ADDR: &str = r#"[{"ifindex":1,"ifname":"lo","flags":["LOOPBACK","UP"],"operstate":"UNKNOWN","address":"00:00:00:00:00:00","addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8}]},
{"ifindex":2,"ifname":"ens18","flags":["UP"],"operstate":"UP","address":"52:54:00:12:34:56","addr_info":[{"family":"inet","local":"192.168.1.5","prefixlen":24},{"family":"inet6","local":"fe80::1","prefixlen":64}]},
{"ifindex":3,"ifname":"wg0","flags":["UP"],"operstate":"UNKNOWN","addr_info":[{"family":"inet","local":"10.8.0.1","prefixlen":24}]},
{"ifindex":4,"ifname":"ens19","flags":[],"operstate":"DOWN","address":"52:54:00:aa:bb:cc","addr_info":[]}]"#;

    #[test]
    fn parsea_ip_addr() {
        let v = parse_ip_addr(IP_ADDR);
        assert_eq!(v.len(), 4);
        assert_eq!(v[1].nombre, "ens18");
        assert_eq!(v[1].mac.as_deref(), Some("52:54:00:12:34:56"));
        assert_eq!(v[1].ips, vec!["192.168.1.5/24".parse().unwrap(), "fe80::1/64".parse().unwrap()]);
        assert!(v[1].activa);
        assert_eq!(v[2].mac, None);
        assert!(v[2].activa, "UNKNOWN con UP cuenta como activa");
        assert!(!v[3].activa);
        assert!(parse_ip_addr("basura").is_empty());
    }

    #[test]
    fn parsea_neigh() {
        let j = r#"[{"dst":"192.168.1.20","dev":"eth1","lladdr":"aa:bb:cc:dd:ee:ff","state":["REACHABLE"]},
{"dst":"192.168.1.21","dev":"eth1","lladdr":"aa:bb:cc:dd:ee:00","state":["STALE"]},
{"dst":"192.168.1.22","dev":"eth1","state":["FAILED"]}]"#;
        let v = parse_neigh(j);
        assert_eq!(v.len(), 2, "sin MAC no cuenta");
        assert!(v[0].conectado);
        assert!(!v[1].conectado);
        assert_eq!(v[0].mac, "aa:bb:cc:dd:ee:ff");
    }

    #[test]
    fn ruta_por_defecto() {
        let t = "Iface\tDestination\tGateway\tFlags\nens18\t0001A8C0\t00000000\t0001\nens18\t00000000\t0101A8C0\t0003\n";
        assert_eq!(parse_ruta_defecto(t).as_deref(), Some("ens18"));
        assert_eq!(parse_ruta_defecto("Iface\tDestination\n"), None);
    }

    #[test]
    fn lee_core_toml() {
        assert_eq!(parse_core_toml("puerto = 9443\nalcance = \"internet\"\n"), (9443, true));
        assert_eq!(parse_core_toml("alcance = \"solo_lan\"\n"), (8443, false));
        assert_eq!(parse_core_toml(""), (8443, false));
        assert_eq!(parse_core_toml("no es toml ["), (8443, false));
    }

    #[test]
    fn lee_wg_conf() {
        assert_eq!(parse_listen_port("[Interface]\nPrivateKey = x\nListenPort = 51821\n\n[Peer]\n"), Some(51821));
        assert_eq!(parse_listen_port("[Interface]\nlistenport=443\n"), Some(443));
        assert_eq!(parse_listen_port("[Interface]\n"), None);
    }

    #[test]
    fn lee_sshd() {
        assert_eq!(parse_sshd("port 2222\nport 22\naddressfamily any\n"), 2222);
        assert_eq!(parse_sshd(""), 22);
    }

    #[test]
    fn fijas_del_sistema() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rutas::en(dir.path());
        std::fs::create_dir_all(&r.etc_netctl).unwrap();
        std::fs::write(r.etc_netctl.join("core.toml"), "puerto = 9443\n").unwrap();
        std::fs::create_dir_all(&r.wireguard).unwrap();
        std::fs::write(r.wireguard.join("wg0.conf"), "[Interface]\nListenPort = 51820\n").unwrap();
        std::fs::write(r.wireguard.join("mal nombre.conf"), "[Interface]\nListenPort = 1\n").unwrap();
        std::fs::write(r.wireguard.join("wg1.conf.bak"), "[Interface]\nListenPort = 2\n").unwrap();
        let s = Falso::new();
        s.responder("sshd -T", Ok("port 2200\n"));
        let f = fijas(&s, &r, &router());
        assert_eq!(f.panel, 9443);
        assert!(!f.alcance_internet);
        assert_eq!(f.ssh, 2200);
        assert_eq!(f.wg, vec![("wg0".to_string(), 51820)]);
        assert_eq!(f.ips_externas, vec!["203.0.113.4".parse::<Ipv4Addr>().unwrap()]);
    }
}
