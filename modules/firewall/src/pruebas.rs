//! Datos de ejemplo para los tests.

use crate::modelo::*;

pub fn iface(nombre: &str, ip: &str) -> Iface {
    Iface { nombre: nombre.into(), ips: vec![ip.parse().unwrap()], mac: None, activa: true }
}

/// Router típico: eth0 a internet, eth1 LAN, wg0 VPN.
pub fn router() -> Config {
    Config::por_defecto(
        &[
            iface("lo", "127.0.0.1/8"),
            iface("eth0", "203.0.113.4/24"),
            iface("eth1", "192.168.1.1/24"),
            iface("wg0", "10.8.0.1/24"),
        ],
        Some("eth0"),
    )
}

pub fn fijas() -> Fijas {
    Fijas {
        panel: 8443,
        alcance_internet: false,
        ssh: 22,
        wg: vec![("wg0".into(), 51820)],
        ips_externas: vec!["203.0.113.4".parse().unwrap()],
    }
}

pub fn reenvio(id: u64, ip: &str, externo: u16) -> Reenvio {
    Reenvio {
        id,
        nombre: format!("r{id}"),
        equipo: Direccion::Ip { red: format!("{ip}/32").parse().unwrap() },
        protocolo: Protocolo::Tcp,
        externo: vec![RangoPuerto::uno(externo)],
        interno: None,
        wan: None,
        desde: vec![],
        caduca: None,
        loopback: true,
        activo: true,
        log: false,
        plantilla: None,
    }
}

pub fn regla(id: u64, o: &str, d: &str, accion: Accion) -> Regla {
    Regla {
        id,
        nombre: format!("regla {id}"),
        origen_zona: o.into(),
        destino_zona: d.into(),
        accion,
        protocolo: Protocolo::Tcp,
        puertos: Puertos::Lista(vec![RangoPuerto::uno(80)]),
        origen: Direccion::Cualquiera,
        destino: Direccion::Cualquiera,
        ..Default::default()
    }
}

pub fn nat(id: u64, tipo: TipoNat) -> ReglaNat {
    ReglaNat {
        id,
        nombre: format!("nat {id}"),
        tipo,
        iface: None,
        origen: Direccion::Cualquiera,
        destino: Direccion::Cualquiera,
        protocolo: Protocolo::Cualquiera,
        puertos: Puertos::Cualquiera,
        activa: true,
        descripcion: String::new(),
    }
}
