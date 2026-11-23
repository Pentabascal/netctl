//! Reglas nftables equivalentes a las de `PostUp` (iptables), en una tabla propia
//! `inet netctl_wg` que se puede recargar de forma atómica con `nft -f`.

use std::fmt::Write as _;
use std::net::Ipv4Addr;

use ipnet::IpNet;

use crate::{Proyecto, nombre_seguro};

const TABLA: &str = "inet netctl_wg";

/// Nombre de interfaz apto para ir entre comillas en nft.
fn interfaz(campo: &str, v: &str) -> Result<String, String> {
    crate::validar_interfaz(campo, v)
}

/// Script completo para `nft -f`: borra y recrea la tabla `inet netctl_wg` de forma atómica.
///
/// Replica la semántica de las reglas iptables de `config_servidor`: con `firewall`, cada
/// cliente activo solo llega a sus destinos (y a internet si lo tiene, sin redes privadas
/// salvo el DNS); el resto del tráfico que entra por la interfaz WireGuard se descarta.
/// El masquerade se añade si `nat` y el proyecto lo tiene activado (`nat = false` cuando
/// otro módulo, p. ej. el firewall, se encarga del NAT).
pub fn tabla_wg(p: &Proyecto, nat: bool) -> Result<String, String> {
    let s = &p.servidor;
    let wg = interfaz("Interfaz WireGuard", &s.interfaz)?;
    let red = p.red_vpn()?;
    let nat = nat && s.nat;
    let out = if nat || s.firewall { interfaz("Interfaz de salida", &s.iface_salida)? } else { String::new() };

    let mut reglas: Vec<String> = Vec::new();
    reglas.push(format!(r#"oifname "{wg}" ct state established,related accept"#));
    if s.firewall {
        if s.entre_clientes {
            reglas.push(format!(r#"iifname "{wg}" ip saddr {red} ip daddr {red} accept"#));
        }
        let dns = p.dns()?;
        for c in p.clientes_activos() {
            let ip: Ipv4Addr =
                c.ip.trim().parse().map_err(|_| format!("«{}»: IP no válida", c.nombre))?;
            let com = format!(r#"comment "{}""#, nombre_seguro(&c.nombre));
            let base = format!(r#"iifname "{wg}" ip saddr {ip}"#);
            for d in p.destinos(c)?.iter().filter(|d| matches!(d, IpNet::V4(_))) {
                reglas.push(format!("{base} ip daddr {d} accept {com}"));
            }
            if c.internet {
                if c.usar_dns {
                    // El DNS configurado se permite aunque esté en una red privada
                    for d in dns.iter().filter(|d| d.is_ipv4()) {
                        for proto in ["udp", "tcp"] {
                            reglas.push(format!("{base} ip daddr {d} {proto} dport 53 accept {com}"));
                        }
                    }
                }
                for b in p.privadas_bloqueadas()?.iter().filter(|b| matches!(b, IpNet::V4(_))) {
                    reglas.push(format!("{base} ip daddr {b} drop {com}"));
                }
                reglas.push(format!(r#"{base} oifname "{out}" accept {com}"#));
            }
        }
        reglas.push(format!(r#"iifname "{wg}" drop"#));
    } else {
        reglas.push(format!(r#"iifname "{wg}" accept"#));
    }

    let mut o = String::new();
    // Crear antes de borrar hace que `delete` no falle si la tabla aún no existe.
    let _ = writeln!(o, "table {TABLA}");
    let _ = writeln!(o, "delete table {TABLA}");
    let _ = writeln!(o, "table {TABLA} {{");
    let _ = writeln!(o, "    chain forward {{");
    let _ = writeln!(o, "        type filter hook forward priority filter - 5; policy accept;");
    for r in &reglas {
        let _ = writeln!(o, "        {r}");
    }
    let _ = writeln!(o, "    }}");
    if nat {
        let _ = writeln!(o, "    chain postrouting {{");
        let _ = writeln!(o, "        type nat hook postrouting priority srcnat; policy accept;");
        // Por cualquier interfaz menos la VPN: internet (salida) y también la intranet,
        // cuyos equipos no saben volver a la red VPN si no se enmascara.
        let _ = writeln!(o, r#"        ip saddr {red} oifname != "{wg}" masquerade"#);
        let _ = writeln!(o, "    }}");
    }
    let _ = writeln!(o, "}}");
    Ok(o)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Proyecto;

    /// Red 10.8.0.0/24, LAN 192.168.1.0/24; cliente1 (10.8.0.2) solo intranet.
    fn proyecto() -> Proyecto {
        let mut p = Proyecto::nuevo();
        let i = p.anadir_cliente();
        assert_eq!(p.clientes[i].ip, "10.8.0.2");
        p
    }

    fn cadena_forward(t: &str) -> Vec<&str> {
        let ini = t.find("chain forward {").expect("falta chain forward");
        let resto = &t[ini..];
        let fin = resto.find('}').unwrap();
        resto[..fin].lines().map(str::trim).filter(|l| !l.is_empty()).collect()
    }

    #[test]
    fn tabla_por_cliente() {
        let t = tabla_wg(&proyecto(), true).unwrap();
        assert!(t.starts_with("table inet netctl_wg\ndelete table inet netctl_wg\n"), "{t}");
        assert!(t.contains("chain forward {\n        type filter hook forward priority filter - 5; policy accept;"), "{t}");
        assert!(t.contains(r#"iifname "wg0" ip saddr 10.8.0.2 ip daddr 192.168.1.0/24 accept"#), "{t}");
        assert_eq!(*cadena_forward(&t).last().unwrap(), r#"iifname "wg0" drop"#, "{t}");
    }

    #[test]
    fn internet_y_dns() {
        let mut p = proyecto();
        let j = p.anadir_cliente();
        p.clientes[j].internet = true;
        p.clientes[j].toda_intranet = false;
        p.clientes[j].usar_dns = true;
        p.servidor.dns = "192.168.1.53".into();
        let t = tabla_wg(&p, true).unwrap();
        assert!(t.contains(r#"iifname "wg0" ip saddr 10.8.0.3 ip daddr 192.168.1.53 udp dport 53 accept"#), "{t}");
        assert!(t.contains(r#"iifname "wg0" ip saddr 10.8.0.3 ip daddr 192.168.0.0/16 drop"#), "{t}");
        assert!(t.contains(r#"iifname "wg0" ip saddr 10.8.0.3 oifname "eth0" accept"#), "{t}");
        assert!(!t.contains("10.8.0.3 ip daddr 192.168.1.0/24 accept"), "{t}");
    }

    #[test]
    fn entre_clientes() {
        let mut p = proyecto();
        assert!(!tabla_wg(&p, true).unwrap().contains("ip saddr 10.8.0.0/24 ip daddr 10.8.0.0/24 accept"));
        p.servidor.entre_clientes = true;
        assert!(tabla_wg(&p, true).unwrap().contains("ip saddr 10.8.0.0/24 ip daddr 10.8.0.0/24 accept"));
    }

    #[test]
    fn nat_opcional() {
        let p = proyecto();
        let con = tabla_wg(&p, true).unwrap();
        assert!(con.contains(r#"ip saddr 10.8.0.0/24 oifname != "wg0" masquerade"#), "{con}");
        assert!(!tabla_wg(&p, false).unwrap().contains("masquerade"));
    }

    #[test]
    fn inactivo_fuera() {
        let mut p = proyecto();
        p.clientes[0].activo = false;
        let t = tabla_wg(&p, true).unwrap();
        assert!(!t.contains("10.8.0.2"), "{t}");
        assert!(!t.contains("cliente1"), "{t}");
        assert!(!p.config_servidor_panel().unwrap().contains(&p.clientes[0].clave_publica));
        assert!(!p.config_servidor().unwrap().contains(&p.clientes[0].clave_publica));
    }

    #[test]
    fn comentario_seguro() {
        let mut p = proyecto();
        p.clientes[0].nombre = "Móvil de Ana".into();
        let t = tabla_wg(&p, true).unwrap();
        assert!(t.contains(r#"comment "M_vil_de_Ana""#), "{t}");
    }

    #[test]
    fn sin_firewall_acepta_todo() {
        let mut p = proyecto();
        p.servidor.firewall = false;
        let t = tabla_wg(&p, true).unwrap();
        assert_eq!(*cadena_forward(&t).last().unwrap(), r#"iifname "wg0" accept"#, "{t}");
    }

    #[test]
    fn servidor_panel_sin_postup() {
        let p = proyecto();
        let c = p.config_servidor_panel().unwrap();
        assert!(!c.contains("PostUp") && !c.contains("PostDown"), "{c}");
        assert!(c.contains("ListenPort = 51820"));
        assert!(c.contains(&p.clientes[0].clave_publica));
        assert!(p.config_servidor().unwrap().contains("PostUp"));
    }

    #[test]
    fn importa_json_antiguo() {
        let json = r#"{
            "servidor": {"interfaz": "wg0", "puerto": 51820, "red_vpn": "10.8.0.0/24"},
            "intranet": [{"id": 1, "nombre": "LAN", "cidr": "192.168.1.0/24"}],
            "clientes": [{"nombre": "viejo", "ip": "10.8.0.2", "keepalive": 25}],
            "sig_id": 1
        }"#;
        let p: Proyecto = serde_json::from_str(json).unwrap();
        assert_eq!(p.servidor.puerto, "51820");
        assert_eq!(p.clientes[0].keepalive, "25");
        assert!(p.clientes[0].activo);
        assert_eq!(p.intranet[0].cidr, "192.168.1.0/24");
    }
}
