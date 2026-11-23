//! Valores por defecto para el asistente, sacados de la propia máquina: interfaz de salida
//! (la de la ruta por defecto) y redes locales privadas conectadas a ella.

use std::net::Ipv4Addr;

use ipnet::Ipv4Net;
use serde::Serialize;

use crate::Rutas;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct Sugerencias {
    pub iface_salida: String,
    pub interfaces: Vec<String>,
    /// Redes privadas conectadas directamente (candidatas a «intranet»).
    pub intranet: Vec<String>,
    pub red_vpn: String,
    pub puerto: String,
    pub dns: String,
}

/// Interfaces que no tiene sentido ofrecer como salida.
fn virtual_(n: &str) -> bool {
    n == "lo"
        || ["wg", "docker", "veth", "virbr", "br-", "tun", "tap"]
            .iter()
            .any(|p| n.starts_with(p))
}

fn hex_ip(h: &str) -> Option<Ipv4Addr> {
    // /proc/net/route da las direcciones en hexadecimal y orden de bytes del host (little endian)
    u32::from_str_radix(h, 16)
        .ok()
        .map(|n| Ipv4Addr::from(n.swap_bytes()))
}

/// (interfaz de la ruta por defecto, redes privadas conectadas sin pasarela).
pub fn de_rutas(texto: &str) -> (Option<String>, Vec<(String, Ipv4Net)>) {
    let mut defecto = None;
    let mut redes = Vec::new();
    for l in texto.lines().skip(1) {
        let c: Vec<&str> = l.split_whitespace().collect();
        if c.len() < 8 || virtual_(c[0]) {
            continue;
        }
        let (Some(dest), Some(gw), Some(mask)) = (hex_ip(c[1]), hex_ip(c[2]), hex_ip(c[7])) else {
            continue;
        };
        if dest.is_unspecified() && mask.is_unspecified() {
            defecto.get_or_insert_with(|| c[0].to_string());
        } else if gw.is_unspecified()
            && dest.is_private()
            && let Ok(n) = Ipv4Net::with_netmask(dest, mask)
        {
            redes.push((c[0].to_string(), n));
        }
    }
    (defecto, redes)
}

pub fn calcular(r: &Rutas) -> Sugerencias {
    let mut interfaces: Vec<String> = std::fs::read_dir(&r.sys_net)
        .map(|d| {
            d.filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|n| !virtual_(n))
                .collect()
        })
        .unwrap_or_default();
    interfaces.sort();
    let (defecto, redes) = de_rutas(&std::fs::read_to_string(&r.proc_route).unwrap_or_default());
    let iface_salida = defecto
        .or_else(|| interfaces.first().cloned())
        .unwrap_or_else(|| "eth0".into());
    let mut intranet: Vec<String> = redes
        .iter()
        .filter(|(i, _)| *i == iface_salida)
        .map(|(_, n)| n.to_string())
        .collect();
    intranet.dedup();
    if !interfaces.contains(&iface_salida) {
        interfaces.insert(0, iface_salida.clone());
    }
    Sugerencias {
        iface_salida,
        interfaces,
        intranet,
        red_vpn: "10.8.0.0/24".into(),
        puerto: "51820".into(),
        dns: "1.1.1.1".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RUTAS: &str = "Iface\tDestination\tGateway \tFlags\tRefCnt\tUse\tMetric\tMask\t\tMTU\tWindow\tIRTT\n\
        enp3s0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
        enp3s0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n\
        docker0\t000011AC\t00000000\t0001\t0\t0\t0\t0000FFFF\t0\t0\t0\n\
        wg0\t0000080A\t00000000\t0001\t0\t0\t0\t00FFFFFF\t0\t0\t0\n";

    #[test]
    fn ruta_por_defecto_y_lan() {
        let (d, redes) = de_rutas(RUTAS);
        assert_eq!(d.as_deref(), Some("enp3s0"));
        assert_eq!(
            redes,
            vec![("enp3s0".to_string(), "192.168.1.0/24".parse().unwrap())]
        );
    }

    #[test]
    fn calcula_con_rutas_falsas() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rutas::en(dir.path());
        std::fs::create_dir_all(r.proc_route.parent().unwrap()).unwrap();
        std::fs::write(&r.proc_route, RUTAS).unwrap();
        for i in ["lo", "enp3s0", "wg0", "eth1"] {
            std::fs::create_dir_all(r.sys_net.join(i)).unwrap();
        }
        let s = calcular(&r);
        assert_eq!(s.iface_salida, "enp3s0");
        assert_eq!(s.interfaces, vec!["enp3s0", "eth1"]);
        assert_eq!(s.intranet, vec!["192.168.1.0/24"]);
    }
}
