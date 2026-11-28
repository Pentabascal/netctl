//! Demo para pruebas visuales (no se empaqueta): el módulo con un sistema falso que no toca
//! nada. Router con internet, LAN, DMZ y VPN, reglas, grupos y reenvíos en varios estados.
//! Todo vive dentro de `<raíz>`.
//!
//!   cargo run -p netctl-mod-firewall --example demo -- <socket> <raíz> [--vacio] [--una] [--firewalld]
//!
//! `--vacio`: sin configurar (muestra el asistente). `--una`: máquina con una sola tarjeta de
//! red. `--firewalld`: firewalld activo (aviso del asistente).

use std::sync::Arc;
use std::time::Duration;

use netctl_mod_firewall::api::{self, Estado};
use netctl_mod_firewall::estado::Almacen;
use netctl_mod_firewall::modelo::*;
use netctl_mod_firewall::registro::{Entrada, EstadoCambio, Registro};
use netctl_mod_firewall::sistema::{Falso, Rutas, Sistema};

/// `Falso` con el reloj de verdad.
struct Demo(Falso);

impl Sistema for Demo {
    fn ejecutar(&self, p: &str, a: &[&str], e: Option<&str>) -> Result<String, String> {
        self.0.ejecutar(p, a, e)
    }
    fn conectar(&self, ip: std::net::IpAddr, puerto: u16) -> bool {
        self.0.conectar(ip, puerto)
    }
}

const ROUTER: &str = r#"[{"ifname":"lo","operstate":"UNKNOWN","flags":["UP"],"address":"00:00:00:00:00:00","addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8}]},
{"ifname":"enp1s0","operstate":"UP","address":"52:54:00:00:00:01","addr_info":[{"family":"inet","local":"85.12.34.56","prefixlen":24}]},
{"ifname":"enp2s0","operstate":"UP","address":"52:54:00:00:00:02","addr_info":[{"family":"inet","local":"192.168.1.1","prefixlen":24}]},
{"ifname":"enp3s0","operstate":"UP","address":"52:54:00:00:00:03","addr_info":[{"family":"inet","local":"172.20.0.1","prefixlen":24}]},
{"ifname":"wg0","operstate":"UNKNOWN","flags":["UP"],"addr_info":[{"family":"inet","local":"10.8.0.1","prefixlen":24}]}]"#;

const UNA: &str = r#"[{"ifname":"lo","operstate":"UNKNOWN","flags":["UP"],"address":"00:00:00:00:00:00","addr_info":[{"family":"inet","local":"127.0.0.1","prefixlen":8}]},
{"ifname":"ens18","operstate":"UP","address":"52:54:00:00:00:01","addr_info":[{"family":"inet","local":"192.168.1.10","prefixlen":24}]},
{"ifname":"wg0","operstate":"UNKNOWN","flags":["UP"],"addr_info":[{"family":"inet","local":"10.8.0.1","prefixlen":24}]}]"#;

const NEIGH: &str = r#"[{"dst":"192.168.1.20","dev":"enp2s0","lladdr":"aa:bb:cc:00:00:20","state":["REACHABLE"]},
{"dst":"192.168.1.21","dev":"enp2s0","lladdr":"aa:bb:cc:00:00:21","state":["REACHABLE"]},
{"dst":"192.168.1.30","dev":"enp2s0","lladdr":"aa:bb:cc:00:00:30","state":["STALE"]},
{"dst":"192.168.1.40","dev":"enp2s0","lladdr":"aa:bb:cc:00:00:40","state":["REACHABLE"]},
{"dst":"172.20.0.10","dev":"enp3s0","lladdr":"aa:bb:cc:00:00:10","state":["REACHABLE"]}]"#;

fn equipo(mac: &str, ip: &str, nombre: &str) -> Direccion {
    Direccion::Equipo { mac: mac.into(), ip: ip.parse().unwrap(), nombre: nombre.into() }
}

fn reenvio(id: u64, nombre: &str, eq: Direccion, proto: Protocolo, puertos: &[(u16, u16)]) -> Reenvio {
    Reenvio {
        id,
        nombre: nombre.into(),
        equipo: eq,
        protocolo: proto,
        externo: puertos.iter().map(|(a, b)| RangoPuerto { desde: *a, hasta: *b }).collect(),
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

fn config(ifaces: &[Iface], una: bool) -> Config {
    let mut c = Config::por_defecto(ifaces, Some(if una { "ens18" } else { "enp1s0" }));
    c.revision = 1;
    if una {
        c.sig_id = 10;
        c.reglas.push(Regla {
            id: 1, nombre: "Servidor web".into(), origen_zona: "externa".into(), destino_zona: "gateway".into(),
            accion: Accion::Permitir, protocolo: Protocolo::Tcp, puertos: Puertos::Lista(vec![RangoPuerto::uno(80), RangoPuerto::uno(443)]),
            origen: Direccion::Cualquiera, destino: Direccion::Cualquiera, log: false, activa: true, ..Default::default()
        });
        return c;
    }
    c.interfaces.insert("enp3s0".into(), "dmz".into());
    c.grupos_ip.push(GrupoIp { id: 1, nombre: "Oficina".into(), miembros: vec![
        Direccion::Ip { red: "198.51.100.0/24".parse().unwrap() },
        Direccion::Rango { desde: "203.0.113.10".parse().unwrap(), hasta: "203.0.113.20".parse().unwrap() },
    ]});
    c.grupos_ip.push(GrupoIp { id: 2, nombre: "Cámaras".into(), miembros: vec![
        equipo("aa:bb:cc:00:00:30", "192.168.1.30", "Cámara entrada"),
    ]});
    c.grupos_puertos.push(GrupoPuertos { id: 3, nombre: "Web".into(), puertos: vec![RangoPuerto::uno(80), RangoPuerto::uno(443)] });
    c.reglas.push(Regla {
        id: 4, nombre: "Cámaras sin internet".into(), origen_zona: "interna".into(), destino_zona: "externa".into(),
        accion: Accion::Bloquear, protocolo: Protocolo::Cualquiera, puertos: Puertos::Cualquiera,
        origen: Direccion::Grupo { id: 2 }, destino: Direccion::Cualquiera, log: false, activa: true,
        descripcion: "Que no manden vídeo a la nube del fabricante".into(), ..Default::default()
    });
    c.reglas.push(Regla {
        id: 5, nombre: "Panel web de la DMZ desde la oficina".into(), origen_zona: "externa".into(), destino_zona: "gateway".into(),
        accion: Accion::Permitir, protocolo: Protocolo::Tcp, puertos: Puertos::Grupo(3),
        origen: Direccion::Grupo { id: 1 }, destino: Direccion::Cualquiera, log: false, activa: true, ..Default::default()
    });
    c.reglas.push(Regla {
        id: 6, nombre: "Impresora solo LAN".into(), origen_zona: "vpn".into(), destino_zona: "interna".into(),
        accion: Accion::Rechazar, protocolo: Protocolo::Tcp, puertos: Puertos::Lista(vec![RangoPuerto::uno(9100)]),
        origen: Direccion::Cualquiera, destino: equipo("aa:bb:cc:00:00:40", "192.168.1.40", "Impresora"), log: false, activa: false, ..Default::default()
    });
    let nas = equipo("aa:bb:cc:00:00:20", "192.168.1.20", "Servidor-casa");
    let mut web = reenvio(7, "Servidor-casa · Web", nas.clone(), Protocolo::Tcp, &[(80, 80), (443, 443)]);
    web.plantilla = Some("web".into());
    let mut mc = reenvio(8, "Servidor-casa · Minecraft", nas.clone(), Protocolo::Tcp, &[(25565, 25565)]);
    mc.plantilla = Some("minecraft".into());
    let mut juegos = reenvio(9, "PC juegos · Servidor de juegos", equipo("aa:bb:cc:00:00:21", "192.168.1.21", "PC juegos"), Protocolo::TcpUdp, &[(27015, 27030)]);
    juegos.plantilla = Some("juegos".into());
    let ahora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
    juegos.caduca = Some(ahora + 3 * 3600);
    let mut rdp = reenvio(10, "Servidor-casa · Escritorio remoto", nas, Protocolo::TcpUdp, &[(3390, 3390)]);
    rdp.interno = Some(3389);
    rdp.plantilla = Some("escritorio_remoto".into());
    rdp.desde = vec![Direccion::Grupo { id: 1 }];
    let mut cam = reenvio(11, "Cámara entrada · Cámaras", equipo("aa:bb:cc:00:00:30", "192.168.1.30", "Cámara entrada"), Protocolo::Tcp, &[(8554, 8554)]);
    cam.interno = Some(554);
    cam.plantilla = Some("camaras".into());
    let mut viejo = reenvio(12, "Portátil · SSH", equipo("aa:bb:cc:00:00:50", "192.168.1.50", "Portátil"), Protocolo::Tcp, &[(2222, 2222)]);
    viejo.interno = Some(22);
    viejo.activo = false;
    viejo.plantilla = Some("ssh".into());
    c.reenvios = vec![web, mc, juegos, rdp, cam, viejo];
    c.sig_id = 12;
    c
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(socket), Some(raiz)) = (args.first(), args.get(1)) else {
        anyhow::bail!("Uso: demo <socket> <raíz> [--vacio] [--una] [--firewalld]");
    };
    let tiene = |x: &str| args.iter().any(|a| a == x);
    let una = tiene("--una");
    let rutas = Rutas::en(std::path::Path::new(raiz));
    std::fs::create_dir_all(rutas.proc_route.parent().unwrap())?;
    let wan = if una { "ens18" } else { "enp1s0" };
    std::fs::write(&rutas.proc_route, format!("Iface\tDestination\tGateway\n{wan}\t00000000\t0101A8C0\n"))?;
    std::fs::create_dir_all(&rutas.wireguard)?;
    std::fs::write(rutas.wireguard.join("wg0.conf"), "[Interface]\nListenPort = 51820\n")?;

    let f = Falso::new();
    let json_ifaces = if una { UNA } else { ROUTER };
    f.responder("ip -j addr", Ok(json_ifaces));
    f.responder("ip -j neigh", Ok(NEIGH));
    f.responder("sshd -T", Ok("port 22\n"));
    f.responder("systemctl is-active", Err("inactive"));
    f.responder("ufw", Err("no existe"));
    if tiene("--firewalld") {
        f.responder("systemctl is-active firewalld", Ok("active\n"));
    }
    f.responder("nft -j list table inet netctl", Ok(r#"{"nftables":[
{"rule":{"comment":"f7_x","expr":[{"counter":{"packets":1532,"bytes":90000}}]}},
{"rule":{"comment":"f8_x","expr":[{"counter":{"packets":87,"bytes":5000}}]}},
{"rule":{"comment":"f9_x","expr":[{"counter":{"packets":4,"bytes":300}}]}},
{"rule":{"comment":"r4_x","expr":[{"counter":{"packets":2210,"bytes":80000}}]}},
{"rule":{"comment":"r5_x","expr":[{"counter":{"packets":12,"bytes":900}}]}},
{"rule":{"comment":"p_externa_gateway","expr":[{"counter":{"packets":48211,"bytes":2900000}}]}}]}"#));
    f.abiertos.lock().unwrap().extend(["192.168.1.20:80".to_string(), "192.168.1.20:25565".into(), "192.168.1.20:3389".into()]);

    if !tiene("--vacio") {
        let ifaces = netctl_mod_firewall::leer::parse_ip_addr(json_ifaces);
        let almacen = Almacen::new(&rutas.datos);
        if almacen.cargar_confirmada().map_err(anyhow::Error::msg)?.is_none() {
            almacen.guardar_confirmada(&config(&ifaces, una)).map_err(anyhow::Error::msg)?;
            let reg = Registro::new(&rutas.datos);
            let ahora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_secs();
            for (hace, resumen, estado) in [
                (86400 * 3, "Firewall configurado con el asistente", EstadoCambio::Confirmado),
                (86400 * 2, "Reenvío de puerto nuevo", EstadoCambio::Confirmado),
                (7200, "Política externa → gateway: Permitir (no se confirmó en 60 s)", EstadoCambio::Revertido),
                (3600, "La IP de «Servidor-casa» cambió de 192.168.1.19 a 192.168.1.20: las reglas se han actualizado.", EstadoCambio::Aplicado),
            ] {
                reg.anotar(Entrada { fecha: ahora - hace, usuario: "admin".into(), resumen: resumen.into(), estado, detalle: None });
            }
        }
    }
    let e = Estado::nuevo(rutas, Arc::new(Demo(f)), Duration::from_secs(60))?;
    e.arrancar().await;
    e.revisar().await;
    tokio::spawn(e.clone().revisar_siempre());
    eprintln!("demo del Firewall en {socket}");
    netctl_sdk::servidor::servir_en(std::path::Path::new(socket), api::router(e)).await
}
