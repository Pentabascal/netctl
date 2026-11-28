//! Imprime el script nft de una configuración de ejemplo completa (para comprobarlo con
//! `unshare -rn nft -c -f -`).
use netctl_mod_firewall::modelo::*;
use netctl_mod_firewall::nft;

fn iface(n: &str, ip: &str) -> Iface {
    Iface { nombre: n.into(), ips: vec![ip.parse().unwrap()], mac: None, activa: true }
}

fn main() {
    let mut c = Config::por_defecto(
        &[iface("eth0", "203.0.113.4/24"), iface("eth1", "192.168.1.1/24"), iface("eth2", "172.20.0.1/24"), iface("wg0", "10.8.0.1/24")],
        Some("eth0"),
    );
    c.interfaces.insert("eth2".into(), "dmz".into());
    c.grupos_ip.push(GrupoIp { id: 1, nombre: "Oficina".into(), miembros: vec![
        Direccion::Ip { red: "198.51.100.0/24".parse().unwrap() },
        Direccion::Rango { desde: "203.0.113.10".parse().unwrap(), hasta: "203.0.113.20".parse().unwrap() },
        Direccion::Ip { red: "2001:db8::/32".parse().unwrap() },
    ]});
    c.grupos_puertos.push(GrupoPuertos { id: 2, nombre: "Web".into(), puertos: vec![RangoPuerto::uno(80), RangoPuerto::uno(443)] });
    c.reglas.push(Regla { id: 3, nombre: "Web desde oficina".into(), origen_zona: "externa".into(), destino_zona: "gateway".into(),
        accion: Accion::Permitir, protocolo: Protocolo::Tcp, puertos: Puertos::Grupo(2), origen: Direccion::Grupo { id: 1 },
        destino: Direccion::Cualquiera, log: true, activa: true, notas: String::new() });
    c.reglas.push(Regla { id: 4, nombre: "DNS fuera".into(), origen_zona: "interna".into(), destino_zona: "externa".into(),
        accion: Accion::Rechazar, protocolo: Protocolo::TcpUdp, puertos: Puertos::Lista(vec![RangoPuerto::uno(53)]),
        origen: Direccion::Equipo { mac: "aa:bb:cc:dd:ee:ff".into(), ip: "192.168.1.50".parse().unwrap(), nombre: "tele".into() },
        destino: Direccion::Cualquiera, log: false, activa: true, notas: String::new() });
    c.reglas.push(Regla { id: 5, nombre: "Ping fuera no".into(), origen_zona: "gateway".into(), destino_zona: "dmz".into(),
        accion: Accion::Bloquear, protocolo: Protocolo::Icmp, puertos: Puertos::Cualquiera, origen: Direccion::Cualquiera,
        destino: Direccion::Ip { red: "fd00::/8".parse().unwrap() }, log: false, activa: true, notas: String::new() });
    c.reenvios.push(Reenvio { id: 6, nombre: "Minecraft".into(), equipo: Direccion::Ip { red: "192.168.1.20/32".parse().unwrap() },
        protocolo: Protocolo::Tcp, externo: vec![RangoPuerto::uno(25565)], interno: None, wan: None, desde: vec![], caduca: None,
        loopback: true, activo: true, log: true, plantilla: None });
    c.reenvios.push(Reenvio { id: 7, nombre: "SSH NAS".into(), equipo: Direccion::Ip { red: "192.168.1.21/32".parse().unwrap() },
        protocolo: Protocolo::Tcp, externo: vec![RangoPuerto::uno(2222)], interno: Some(22), wan: Some("eth0".into()),
        desde: vec![Direccion::Ip { red: "198.51.100.7/32".parse().unwrap() }, Direccion::Grupo { id: 1 }], caduca: None,
        loopback: true, activo: true, log: false, plantilla: None });
    c.reenvios.push(Reenvio { id: 8, nombre: "Juegos".into(), equipo: Direccion::Ip { red: "192.168.1.22/32".parse().unwrap() },
        protocolo: Protocolo::TcpUdp, externo: vec![RangoPuerto { desde: 27015, hasta: 27030 }], interno: None, wan: None,
        desde: vec![], caduca: None, loopback: true, activo: true, log: false, plantilla: None });
    let f = Fijas { panel: 8443, alcance_internet: false, ssh: 22, wg: vec![("wg0".into(), 51820)], ips_externas: vec!["203.0.113.4".parse().unwrap()] };
    print!("{}", nft::generar(&c, &f, 0).unwrap());
}
