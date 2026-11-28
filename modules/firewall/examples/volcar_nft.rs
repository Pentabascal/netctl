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
        destino: Direccion::Cualquiera, log: true, activa: true, ..Default::default() });
    c.reglas.push(Regla { id: 4, nombre: "DNS fuera".into(), origen_zona: "interna".into(), destino_zona: "externa".into(),
        accion: Accion::Rechazar, protocolo: Protocolo::TcpUdp, puertos: Puertos::Lista(vec![RangoPuerto::uno(53)]),
        origen: Direccion::Equipo { mac: "aa:bb:cc:dd:ee:ff".into(), ip: "192.168.1.50".parse().unwrap(), nombre: "tele".into() },
        destino: Direccion::Cualquiera, log: false, activa: true, ..Default::default() });
    c.reglas.push(Regla { id: 5, nombre: "Ping fuera no".into(), origen_zona: "gateway".into(), destino_zona: "dmz".into(),
        accion: Accion::Bloquear, protocolo: Protocolo::Icmp, puertos: Puertos::Cualquiera, origen: Direccion::Cualquiera,
        destino: Direccion::Ip { red: "fd00::/8".parse().unwrap() }, log: false, activa: true, ..Default::default() });
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
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "hotspot") {
        c.interfaces.insert("eth2".into(), "hotspot".into());
    }
    if args.iter().any(|a| a == "nuevos") {
        let base = |id: u64, o: &str, d: &str| Regla { id, nombre: format!("n{id}"), origen_zona: o.into(), destino_zona: d.into(), ..Default::default() };
        let mut r = base(10, "interna", "externa");
        r.origen = Direccion::Interfaz { iface: "eth1".into() };
        r.protocolo = Protocolo::TcpUdp;
        r.puertos_origen = Puertos::Lista(vec![RangoPuerto { desde: 1024, hasta: 65535 }]);
        r.puertos = Puertos::Lista(vec![RangoPuerto::uno(53)]);
        r.version_ip = VersionIp::V4;
        r.horario = Horario::Semanal { dias: vec![0, 1, 2, 3, 4], franja: Franja { desde: "08:00".into(), hasta: "17:00".into() } };
        c.reglas.push(r);
        let mut r = base(11, "interna", "gateway");
        r.origen = Direccion::Mac { mac: "aa:bb:cc:dd:ee:01".into() };
        r.protocolo = Protocolo::Tcp;
        r.puertos = Puertos::Lista(vec![RangoPuerto::uno(22)]);
        r.estado = EstadoConexion::Elegir(vec![Estado::Establecida, Estado::Relacionada]);
        c.reglas.push(r);
        let mut r = base(12, "dmz", "externa");
        r.protocolo = Protocolo::Otro;
        r.numero_protocolo = 47;
        r.accion = Accion::Permitir;
        r.horario = Horario::AMedida(vec![
            Tramo { dias: vec![5, 6], franja: Franja { desde: "22:00".into(), hasta: "06:00".into() } },
            Tramo { dias: vec![2], franja: Franja { desde: "10:00".into(), hasta: "11:30".into() } },
        ]);
        c.reglas.push(r);
        let mut r = base(13, "interna", "dmz");
        r.version_ip = VersionIp::V6;
        r.estado = EstadoConexion::SoloNuevas;
        r.horario = Horario::UnaVez { inicio: 1_700_000_000, fin: 4_000_000_000 };
        r.destino = Direccion::Interfaz { iface: "eth2".into() };
        c.reglas.push(r);
        let mut r = base(14, "gateway", "externa");
        r.protocolo = Protocolo::Udp;
        r.puertos_origen = Puertos::Grupo(2);
        r.horario = Horario::Diario(Franja { desde: "00:00".into(), hasta: "23:59".into() });
        c.reglas.push(r);
    }
    if args.iter().any(|a| a == "nat") {
        let base = |id: u64, tipo: TipoNat| ReglaNat { id, nombre: format!("nat{id}"), tipo, iface: None, origen: Direccion::Cualquiera,
            destino: Direccion::Cualquiera, protocolo: Protocolo::Cualquiera, puertos: Puertos::Cualquiera, activa: true, descripcion: String::new() };
        let mut n = base(20, TipoNat::Masquerade);
        n.iface = Some("eth0".into());
        c.nat_propio.push(n);
        let mut n = base(21, TipoNat::Snat { ip: "203.0.113.5".parse().unwrap() });
        n.iface = Some("eth0".into());
        n.origen = Direccion::Grupo { id: 1 };
        c.nat_propio.push(n);
        let mut n = base(22, TipoNat::Dnat { ip: "192.168.1.10".parse().unwrap(), puerto: Some(8080) });
        n.iface = Some("eth0".into());
        n.protocolo = Protocolo::TcpUdp;
        n.puertos = Puertos::Grupo(2);
        c.nat_propio.push(n);
        let mut n = base(23, TipoNat::Dnat { ip: "fd00::10".parse().unwrap(), puerto: None });
        n.origen = Direccion::Mac { mac: "aa:bb:cc:dd:ee:01".into() };
        n.protocolo = Protocolo::Udp;
        c.nat_propio.push(n);
    }
    let f = Fijas { panel: 8443, alcance_internet: false, ssh: 22, wg: vec![("wg0".into(), 51820)], ips_externas: vec!["203.0.113.4".parse().unwrap()] };
    print!("{}", nft::generar(&c, &f, 1_800_000_000).unwrap());
}
