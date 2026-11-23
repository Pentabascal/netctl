//! Qué orígenes pueden entrar al panel y en qué direcciones se escucha.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

use ipnet::IpNet;

use crate::config::Alcance;

/// Dirección privada o local: RFC1918, 127/8, 169.254/16, ::1, fc00::/7, fe80::/10.
pub fn privada(ip: IpAddr) -> bool {
    match ip.to_canonical() {
        IpAddr::V4(v4) => v4.is_private() || v4.is_loopback() || v4.is_link_local(),
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// ¿Se acepta una conexión desde `origen`? En `SoloLan` solo privadas y las redes de las
/// interfaces WireGuard (`ifaces_wg`).
pub fn permitida(alcance: Alcance, origen: IpAddr, ifaces_wg: &[IpNet]) -> bool {
    let origen = origen.to_canonical();
    match alcance {
        Alcance::Internet => true,
        Alcance::SoloLan => privada(origen) || ifaces_wg.iter().any(|r| r.contains(&origen)),
    }
}

/// Interfaz de red con su dirección y si es WireGuard.
#[derive(Clone, Debug)]
pub struct Interfaz {
    pub nombre: String,
    pub red: IpNet,
    pub wireguard: bool,
}

/// Direcciones en las que escuchar. `Internet` → todas (`::` y `0.0.0.0`); `SoloLan` → las IP
/// privadas o de WireGuard de las interfaces, más `127.0.0.1`.
pub fn direcciones_escucha(alcance: Alcance, ifaces: &[Interfaz]) -> Vec<IpAddr> {
    if alcance == Alcance::Internet {
        return vec![IpAddr::V6(Ipv6Addr::UNSPECIFIED)];
    }
    let mut v: Vec<IpAddr> = Vec::new();
    for i in ifaces {
        let ip = i.red.addr();
        // Las fe80:: necesitan el índice de interfaz para enlazar: se omiten.
        let enlace_local_v6 = matches!(ip, IpAddr::V6(v6) if (v6.segments()[0] & 0xffc0) == 0xfe80);
        if (i.wireguard || privada(ip)) && !ip.is_loopback() && !enlace_local_v6 && !v.contains(&ip)
        {
            v.push(ip);
        }
    }
    // Siempre en 127.0.0.1: permite entrar por un túnel SSH.
    v.push(IpAddr::V4(Ipv4Addr::LOCALHOST));
    v
}

/// Interfaces del equipo (sin `lo`). Una interfaz es WireGuard si su `uevent` lo dice.
pub fn interfaces() -> Vec<Interfaz> {
    let Ok(lista) = if_addrs::get_if_addrs() else {
        return vec![];
    };
    lista
        .into_iter()
        .filter(|i| !i.is_loopback())
        .filter_map(|i| {
            let red = match i.addr {
                if_addrs::IfAddr::V4(a) => {
                    IpNet::with_netmask(IpAddr::V4(a.ip), IpAddr::V4(a.netmask)).ok()?
                }
                if_addrs::IfAddr::V6(a) => {
                    IpNet::with_netmask(IpAddr::V6(a.ip), IpAddr::V6(a.netmask)).ok()?
                }
            };
            let wireguard = std::fs::read_to_string(format!("/sys/class/net/{}/uevent", i.name))
                .map(|t| t.lines().any(|l| l.trim() == "DEVTYPE=wireguard"))
                .unwrap_or(false);
            Some(Interfaz {
                nombre: i.name,
                red,
                wireguard,
            })
        })
        .collect()
}

/// Redes de las interfaces WireGuard.
pub fn redes_wg(ifaces: &[Interfaz]) -> Vec<IpNet> {
    ifaces
        .iter()
        .filter(|i| i.wireguard)
        .map(|i| i.red.trunc())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn alcance_solo_lan() {
        let wg: Vec<IpNet> = vec![
            "10.8.0.0/24".parse().unwrap(),
            "100.64.10.0/24".parse().unwrap(),
        ];
        assert!(!permitida(Alcance::SoloLan, ip("8.8.8.8"), &wg));
        assert!(permitida(Alcance::SoloLan, ip("192.168.1.5"), &wg));
        assert!(permitida(Alcance::SoloLan, ip("10.8.0.7"), &wg));
        assert!(
            permitida(Alcance::SoloLan, ip("100.64.10.3"), &wg),
            "red wg no privada"
        );
        assert!(!permitida(Alcance::SoloLan, ip("100.64.11.3"), &wg));
        assert!(permitida(Alcance::SoloLan, ip("127.0.0.1"), &wg));
        assert!(permitida(Alcance::SoloLan, ip("fd00::1"), &wg));
        assert!(permitida(Alcance::SoloLan, ip("fe80::1"), &wg));
        assert!(!permitida(Alcance::SoloLan, ip("2001:db8::1"), &wg));
        // IPv4 mapeada en IPv6 (sockets duales): se mira la IPv4.
        assert!(!permitida(Alcance::SoloLan, ip("::ffff:8.8.8.8"), &wg));
        assert!(permitida(Alcance::SoloLan, ip("::ffff:192.168.1.5"), &wg));
        assert!(permitida(Alcance::Internet, ip("8.8.8.8"), &wg));
    }

    #[test]
    fn escucha_segun_alcance() {
        let ifaces = vec![
            Interfaz {
                nombre: "eth0".into(),
                red: "192.168.1.10/24".parse().unwrap(),
                wireguard: false,
            },
            Interfaz {
                nombre: "eth1".into(),
                red: "203.0.113.5/24".parse().unwrap(),
                wireguard: false,
            },
            Interfaz {
                nombre: "wg0".into(),
                red: "100.64.10.1/24".parse().unwrap(),
                wireguard: true,
            },
        ];
        let d = direcciones_escucha(Alcance::SoloLan, &ifaces);
        assert_eq!(
            d,
            vec![ip("192.168.1.10"), ip("100.64.10.1"), ip("127.0.0.1")]
        );
        assert_eq!(
            direcciones_escucha(Alcance::SoloLan, &ifaces[1..2]),
            vec![ip("127.0.0.1")]
        );
        assert_eq!(
            direcciones_escucha(Alcance::Internet, &ifaces),
            vec![ip("::")]
        );
        assert_eq!(
            redes_wg(&ifaces),
            vec!["100.64.10.0/24".parse::<IpNet>().unwrap()]
        );
    }
}
