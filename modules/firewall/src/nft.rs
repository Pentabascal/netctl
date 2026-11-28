//! Generación del script de `nft -f` para la tabla `inet netctl`.
//!
//! Todo lo que viene del usuario pasa por tipos ya validados (IPs, puertos, ids numéricos) o
//! por [`crate::nombre_seguro`] (comentarios), así que nada se interpola tal cual.

use std::collections::BTreeSet;
use std::fmt::Write as _;

use crate::modelo::*;
use crate::nombre_seguro;
use crate::validar::{ip_reenvio, validar};

const PRIVADAS4: &str = "{ 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8, 169.254.0.0/16 }";
const PRIVADAS6: &str = "{ fc00::/7, fe80::/10, ::1 }";

#[derive(Clone, Copy, PartialEq)]
enum Fam {
    Ambas,
    V4,
    V6,
}

fn lista_ifaces(v: &[String]) -> String {
    let partes: Vec<String> = v.iter().map(|i| format!("\"{i}\"")).collect();
    format!("{{ {} }}", partes.join(", "))
}

fn rango_txt(r: &RangoPuerto) -> String {
    if r.desde == r.hasta { r.desde.to_string() } else { format!("{}-{}", r.desde, r.hasta) }
}

fn puertos_txt(l: &[RangoPuerto]) -> String {
    if l.len() == 1 {
        rango_txt(&l[0])
    } else {
        format!("{{ {} }}", l.iter().map(rango_txt).collect::<Vec<_>>().join(", "))
    }
}

/// `tcp dport X` / `udp dport X` / `meta l4proto { tcp, udp } th dport X`; `X` vacío = sin puerto.
fn proto_txt(p: Protocolo, dport: Option<&str>) -> String {
    match (p, dport) {
        (Protocolo::Cualquiera, _) => String::new(),
        (Protocolo::Icmp, _) => "meta l4proto { icmp, ipv6-icmp }".into(),
        (Protocolo::Tcp, Some(x)) => format!("tcp dport {x}"),
        (Protocolo::Udp, Some(x)) => format!("udp dport {x}"),
        (Protocolo::TcpUdp, Some(x)) => format!("meta l4proto {{ tcp, udp }} th dport {x}"),
        (Protocolo::Tcp, None) => "meta l4proto tcp".into(),
        (Protocolo::Udp, None) => "meta l4proto udp".into(),
        (Protocolo::TcpUdp, None) => "meta l4proto { tcp, udp }".into(),
        // Provisional: la Task 4 lo escribe con su número.
        (Protocolo::Otro, _) => String::new(),
    }
}

fn elemento(d: &Direccion) -> Option<(bool, String)> {
    match d {
        Direccion::Equipo { ip, .. } => Some((ip.is_ipv4(), ip.to_string())),
        Direccion::Ip { red } => Some((red.addr().is_ipv4(), red.to_string())),
        Direccion::Rango { desde, hasta } => Some((desde.is_ipv4(), format!("{desde}-{hasta}"))),
        _ => None,
    }
}

/// Elementos IPv4 e IPv6 de un grupo de IPs.
fn elementos_grupo(g: &GrupoIp) -> (Vec<String>, Vec<String>) {
    let (mut v4, mut v6) = (vec![], vec![]);
    for (es4, e) in g.miembros.iter().filter_map(elemento) {
        if es4 { v4.push(e) } else { v6.push(e) }
    }
    (v4, v6)
}

/// Variantes de la condición de dirección: (familia, texto). Vacío = no coincide con nada.
fn dir_txt(c: &Config, d: &Direccion, lado: &str) -> Vec<(Fam, String)> {
    match d {
        Direccion::Cualquiera => vec![(Fam::Ambas, String::new())],
        Direccion::Grupo { id } => {
            let Some(g) = c.grupos_ip.iter().find(|g| g.id == *id) else { return vec![] };
            let (v4, v6) = elementos_grupo(g);
            let mut v = vec![];
            if !v4.is_empty() {
                v.push((Fam::V4, format!("ip {lado} @gi{id}_4")));
            }
            if !v6.is_empty() {
                v.push((Fam::V6, format!("ip6 {lado} @gi{id}_6")));
            }
            v
        }
        otro => {
            let (es4, e) = elemento(otro).expect("dirección simple");
            if es4 {
                vec![(Fam::V4, format!("ip {lado} {e}"))]
            } else {
                vec![(Fam::V6, format!("ip6 {lado} {e}"))]
            }
        }
    }
}

fn linea(t: &mut String, partes: &[&str]) {
    let p: Vec<&str> = partes.iter().copied().filter(|x| !x.is_empty()).collect();
    let _ = writeln!(t, "\t\t{}", p.join(" "));
}

fn accion_txt(a: Accion) -> &'static str {
    match a {
        Accion::Permitir => "accept",
        Accion::Bloquear => "drop",
        Accion::Rechazar => "reject",
    }
}

/// Reglas del usuario de una casilla: `iif`/`oif` ya resueltos.
fn reglas_usuario(t: &mut String, c: &Config, r: &Regla, iif: &str, oif: &str) {
    let dport = match &r.puertos {
        Puertos::Cualquiera => None,
        Puertos::Lista(l) => Some(puertos_txt(l)),
        Puertos::Grupo(id) => {
            if !c.grupos_puertos.iter().any(|g| g.id == *id && !g.puertos.is_empty()) {
                return;
            }
            Some(format!("@gp{id}"))
        }
    };
    let proto = proto_txt(r.protocolo, dport.as_deref());
    let log = if r.log { format!("log prefix \"netctl r{} \"", r.id) } else { String::new() };
    let comentario = format!("comment \"r{}_{}\"", r.id, nombre_seguro(&r.nombre));
    for (fo, o) in dir_txt(c, &r.origen, "saddr") {
        for (fd, d) in dir_txt(c, &r.destino, "daddr") {
            if fo != Fam::Ambas && fd != Fam::Ambas && fo != fd {
                continue;
            }
            linea(t, &[iif, oif, &o, &d, &proto, "counter", &log, accion_txt(r.accion), &comentario]);
        }
    }
}

/// Script completo (idempotente) o la lista de errores de validación.
pub fn generar(c: &Config, f: &Fijas, ahora: u64) -> Result<String, Vec<String>> {
    let errores = validar(c, f);
    if !errores.is_empty() {
        return Err(errores);
    }
    let gateway: BTreeSet<String> = c.zonas_tipo(TipoZona::Gateway).into_iter().collect();
    let ext = c.ifaces_externas();
    let mut wg_ifaces: Vec<String> = c.zonas_tipo(TipoZona::Vpn).iter().flat_map(|z| c.ifaces_de(z)).collect();
    for (i, _) in &f.wg {
        if !wg_ifaces.contains(i) {
            wg_ifaces.push(i.clone());
        }
    }

    let mut t = String::from("table inet netctl {}\ndelete table inet netctl\ntable inet netctl {\n");

    // Conjuntos (grupos)
    for g in &c.grupos_ip {
        let (v4, v6) = elementos_grupo(g);
        if !v4.is_empty() {
            let _ = writeln!(t, "\tset gi{}_4 {{ type ipv4_addr; flags interval; elements = {{ {} }} }}", g.id, v4.join(", "));
        }
        if !v6.is_empty() {
            let _ = writeln!(t, "\tset gi{}_6 {{ type ipv6_addr; flags interval; elements = {{ {} }} }}", g.id, v6.join(", "));
        }
    }
    for g in c.grupos_puertos.iter().filter(|g| !g.puertos.is_empty()) {
        let l: Vec<_> = g.puertos.iter().map(rango_txt).collect();
        let _ = writeln!(t, "\tset gp{} {{ type inet_service; flags interval; elements = {{ {} }} }}", g.id, l.join(", "));
    }

    let reglas: Vec<&Regla> = c.reglas.iter().filter(|r| r.activa).collect();
    let zonas_con_ifaces: Vec<(&Zona, Vec<String>)> = c
        .zonas
        .iter()
        .filter(|z| !gateway.contains(&z.id))
        .map(|z| (z, c.ifaces_de(&z.id)))
        .filter(|(_, i)| !i.is_empty())
        .collect();

    // ---- input: hacia esta máquina
    t.push_str("\tchain input {\n\t\ttype filter hook input priority filter; policy drop;\n");
    t.push_str("\t\tct state established,related accept\n\t\tct state invalid drop\n\t\tiif lo accept\n");
    t.push_str("\t\ticmpv6 type { nd-neighbor-solicit, nd-neighbor-advert, nd-router-solicit, nd-router-advert, destination-unreachable, packet-too-big, time-exceeded, parameter-problem } accept\n");
    t.push_str("\t\ticmp type { destination-unreachable, time-exceeded, parameter-problem } accept\n");
    t.push_str("\t\tudp sport 67 udp dport 68 accept comment \"fija_dhcp\"\n");
    t.push_str("\t\tip6 saddr fe80::/10 udp sport 547 udp dport 546 accept comment \"fija_dhcpv6\"\n");
    if f.alcance_internet {
        let _ = writeln!(t, "\t\ttcp dport {} accept comment \"fija_panel\"", f.panel);
    } else {
        let _ = writeln!(t, "\t\tip saddr {PRIVADAS4} tcp dport {} accept comment \"fija_panel\"", f.panel);
        let _ = writeln!(t, "\t\tip6 saddr {PRIVADAS6} tcp dport {} accept comment \"fija_panel\"", f.panel);
        if !wg_ifaces.is_empty() {
            let _ = writeln!(t, "\t\tiifname {} tcp dport {} accept comment \"fija_panel\"", lista_ifaces(&wg_ifaces), f.panel);
        }
    }
    let _ = writeln!(t, "\t\tip saddr {PRIVADAS4} tcp dport {} accept comment \"fija_ssh\"", f.ssh);
    let _ = writeln!(t, "\t\tip6 saddr {PRIVADAS6} tcp dport {} accept comment \"fija_ssh\"", f.ssh);
    if !wg_ifaces.is_empty() {
        let _ = writeln!(t, "\t\tiifname {} tcp dport {} accept comment \"fija_ssh\"", lista_ifaces(&wg_ifaces), f.ssh);
    }
    for (i, p) in &f.wg {
        let _ = writeln!(t, "\t\tudp dport {p} accept comment \"fija_wg_{}\"", nombre_seguro(i));
    }
    if c.ping_externo || ext.is_empty() {
        t.push_str("\t\ticmp type echo-request accept\n\t\ticmpv6 type echo-request accept\n");
    } else {
        let l = lista_ifaces(&ext);
        let _ = writeln!(t, "\t\tiifname != {l} icmp type echo-request accept\n\t\tiifname != {l} icmpv6 type echo-request accept");
    }
    for r in reglas.iter().filter(|r| gateway.contains(&r.destino_zona) && !gateway.contains(&r.origen_zona)) {
        let ifs = c.ifaces_de(&r.origen_zona);
        if !ifs.is_empty() {
            reglas_usuario(&mut t, c, r, &format!("iifname {}", lista_ifaces(&ifs)), "");
        }
    }
    for g in &gateway {
        for (z, ifs) in &zonas_con_ifaces {
            let a = if c.politica(&z.id, g) == Politica::Permitir { "accept" } else { "drop" };
            let _ = writeln!(t, "\t\tiifname {} counter {a} comment \"p_{}_{g}\"", lista_ifaces(ifs), z.id);
        }
    }
    t.push_str("\t}\n");

    // ---- forward: entre zonas
    t.push_str("\tchain forward {\n\t\ttype filter hook forward priority filter; policy drop;\n");
    t.push_str("\t\tct state established,related accept\n\t\tct state invalid drop\n\t\tct status dnat accept\n");
    for r in reglas.iter().filter(|r| !gateway.contains(&r.destino_zona) && !gateway.contains(&r.origen_zona)) {
        let (o, d) = (c.ifaces_de(&r.origen_zona), c.ifaces_de(&r.destino_zona));
        if !o.is_empty() && !d.is_empty() {
            reglas_usuario(&mut t, c, r, &format!("iifname {}", lista_ifaces(&o)), &format!("oifname {}", lista_ifaces(&d)));
        }
    }
    for (zo, io) in &zonas_con_ifaces {
        for (zd, id) in &zonas_con_ifaces {
            let a = if c.politica(&zo.id, &zd.id) == Politica::Permitir { "accept" } else { "drop" };
            let _ = writeln!(t, "\t\tiifname {} oifname {} counter {a} comment \"p_{}_{}\"", lista_ifaces(io), lista_ifaces(id), zo.id, zd.id);
        }
    }
    t.push_str("\t}\n");

    // ---- output: desde esta máquina
    t.push_str("\tchain output {\n\t\ttype filter hook output priority filter; policy accept;\n");
    t.push_str("\t\tct state established,related accept\n");
    for r in reglas.iter().filter(|r| gateway.contains(&r.origen_zona) && !gateway.contains(&r.destino_zona)) {
        let ifs = c.ifaces_de(&r.destino_zona);
        if !ifs.is_empty() {
            reglas_usuario(&mut t, c, r, "", &format!("oifname {}", lista_ifaces(&ifs)));
        }
    }
    for g in &gateway {
        for (z, ifs) in &zonas_con_ifaces {
            if c.politica(g, &z.id) == Politica::Bloquear {
                let _ = writeln!(t, "\t\toifname {} counter drop comment \"p_{g}_{}\"", lista_ifaces(ifs), z.id);
            }
        }
    }
    t.push_str("\t}\n");

    // ---- NAT: reenvíos (DNAT) y enmascaramiento
    let reenvios: Vec<&Reenvio> = c
        .reenvios
        .iter()
        .filter(|r| r.activo && r.caduca.is_none_or(|x| x > ahora))
        .collect();
    let mut pre = String::new();
    let mut post = String::new();
    for r in &reenvios {
        let Some(ip) = ip_reenvio(r) else { continue };
        let wans = match &r.wan {
            Some(w) => vec![w.clone()],
            None => ext.clone(),
        };
        if wans.is_empty() {
            continue;
        }
        let wl = lista_ifaces(&wans);
        let proto = proto_txt(r.protocolo, Some(&puertos_txt(&r.externo)));
        let destino = match r.interno {
            Some(p) => format!("dnat ip to {ip}:{p}"),
            None => format!("dnat ip to {ip}"),
        };
        let log = if r.log { format!("log prefix \"netctl f{} \"", r.id) } else { String::new() };
        let comentario = format!("comment \"f{}_{}\"", r.id, nombre_seguro(&r.nombre));
        let iif = format!("iifname {wl}");
        // desde: lista literal (IPv4) + un conjunto por grupo
        let mut origenes: Vec<String> = vec![];
        let literales: Vec<String> = r
            .desde
            .iter()
            .filter_map(elemento)
            .filter(|(es4, _)| *es4)
            .map(|(_, e)| e)
            .collect();
        if !literales.is_empty() {
            origenes.push(format!("ip saddr {{ {} }}", literales.join(", ")));
        }
        for d in &r.desde {
            if let Direccion::Grupo { id } = d
                && c.grupos_ip.iter().any(|g| g.id == *id && !elementos_grupo(g).0.is_empty())
            {
                origenes.push(format!("ip saddr @gi{id}_4"));
            }
        }
        if r.desde.is_empty() {
            origenes.push(String::new());
        }
        for o in &origenes {
            let mut l = String::new();
            linea(&mut l, &[&iif, o, &proto, "counter", &log, &destino, &comentario]);
            pre.push_str(&l);
        }
        if r.loopback && !f.ips_externas.is_empty() {
            let ips: Vec<String> = f.ips_externas.iter().map(|i| i.to_string()).collect();
            let mut l = String::new();
            linea(&mut l, &[&format!("iifname != {wl}"), &format!("ip daddr {{ {} }}", ips.join(", ")), &proto, "counter", &destino, &comentario]);
            pre.push_str(&l);
            let interno = match r.interno {
                Some(p) => p.to_string(),
                None => puertos_txt(&r.externo),
            };
            let proto_in = proto_txt(r.protocolo, Some(&interno));
            let mut l = String::new();
            linea(&mut l, &[&format!("iifname != {wl}"), &format!("ip daddr {ip}"), &proto_in, "ct status dnat masquerade", &comentario]);
            post.push_str(&l);
        }
    }
    t.push_str("\tchain prerouting {\n\t\ttype nat hook prerouting priority dstnat; policy accept;\n");
    t.push_str(&pre);
    t.push_str("\t}\n");
    t.push_str("\tchain postrouting {\n\t\ttype nat hook postrouting priority srcnat; policy accept;\n");
    t.push_str(&post);
    if !ext.is_empty() {
        for (z, ifs) in &zonas_con_ifaces {
            if z.tipo != TipoZona::Externa && c.nat.get(&z.id).copied().unwrap_or(false) {
                let _ = writeln!(t, "\t\tiifname {} oifname {} masquerade comment \"nat_{}\"", lista_ifaces(ifs), lista_ifaces(&ext), z.id);
            }
        }
    }
    t.push_str("\t}\n}\n");
    Ok(t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::*;

    fn generado(c: &Config) -> String {
        generar(c, &fijas(), 1000).unwrap()
    }

    /// Posición de `aguja` en `texto` (falla con mensaje si no está).
    fn pos(texto: &str, aguja: &str) -> usize {
        texto.find(aguja).unwrap_or_else(|| panic!("no está «{aguja}» en:\n{texto}"))
    }

    #[test]
    fn cabecera_idempotente() {
        let t = generado(&router());
        assert!(t.starts_with("table inet netctl {}\ndelete table inet netctl\ntable inet netctl {"), "{t}");
        assert!(t.contains("type filter hook input priority filter; policy drop;"));
        assert!(t.contains("type filter hook forward priority filter; policy drop;"));
        assert!(t.contains("type filter hook output priority filter; policy accept;"));
        assert!(t.contains("type nat hook prerouting priority dstnat; policy accept;"));
        assert!(t.contains("type nat hook postrouting priority srcnat; policy accept;"));
        assert!(!t.contains("netctl_wg"));
    }

    #[test]
    fn fijas_primero() {
        let mut c = router();
        for o in ["externa", "interna", "vpn", "dmz"] {
            c.politicas.push(PoliticaPar { origen: o.into(), destino: "gateway".into(), politica: Politica::Bloquear });
        }
        let mut r = regla(1, "interna", "gateway", Accion::Bloquear);
        r.puertos = Puertos::Cualquiera;
        r.protocolo = Protocolo::Cualquiera;
        c.reglas.push(r);
        let t = generado(&c);
        let input = &t[pos(&t, "chain input")..pos(&t, "chain forward")];
        let est = pos(input, "ct state established,related accept");
        let panel = pos(input, "tcp dport 8443 accept comment \"fija_panel\"");
        let ssh = pos(input, "tcp dport 22 accept comment \"fija_ssh\"");
        let wg = pos(input, "udp dport 51820 accept comment \"fija_wg_wg0\"");
        let usuario = pos(input, "comment \"r1_");
        assert!(est < panel && panel < usuario && ssh < usuario && wg < usuario);
        // solo LAN/VPN: el panel solo desde redes privadas e interfaces wg
        assert!(input.contains("ip saddr { 10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16, 127.0.0.0/8, 169.254.0.0/16 } tcp dport 8443 accept"));
        assert!(input.contains("iifname { \"wg0\" } tcp dport 8443 accept"));
    }

    #[test]
    fn panel_internet_abierto() {
        let mut f = fijas();
        f.alcance_internet = true;
        let t = generar(&router(), &f, 0).unwrap();
        assert!(t.contains("\n\t\ttcp dport 8443 accept comment \"fija_panel\""), "{t}");
    }

    #[test]
    fn ipv6_icmp_necesario() {
        let t = generado(&router());
        assert!(t.contains("icmpv6 type { nd-neighbor-solicit, nd-neighbor-advert, nd-router-solicit, nd-router-advert, destination-unreachable, packet-too-big, time-exceeded, parameter-problem } accept"));
    }

    #[test]
    fn politicas_en_cadenas() {
        let t = generado(&router());
        let input = &t[pos(&t, "chain input")..pos(&t, "chain forward")];
        // Interna→Gateway permitir; Externa→Gateway cae en la política drop
        assert!(input.contains("iifname { \"eth1\" } counter accept comment \"p_interna_gateway\""), "{input}");
        assert!(input.contains("iifname { \"eth0\" } counter drop comment \"p_externa_gateway\""), "{input}");
        let fwd = &t[pos(&t, "chain forward")..pos(&t, "chain output")];
        assert!(fwd.contains("iifname { \"eth1\" } oifname { \"eth0\" } counter accept comment \"p_interna_externa\""), "{fwd}");
        assert!(fwd.contains("iifname { \"eth0\" } oifname { \"eth1\" } counter drop comment \"p_externa_interna\""), "{fwd}");
        assert!(fwd.contains("ct status dnat accept"));
        // zonas sin interfaces (dmz) no generan reglas vacías
        assert!(!t.contains("{  }") && !t.contains("{ }"));
    }

    #[test]
    fn ping_externo() {
        let mut c = router();
        assert!(generado(&c).contains("icmp type echo-request accept"));
        c.ping_externo = false;
        let t = generado(&c);
        assert!(!t.contains("\t\ticmp type echo-request accept"));
        assert!(t.contains("iifname != { \"eth0\" } icmp type echo-request accept"));
    }

    #[test]
    fn regla_usuario() {
        let mut c = router();
        let mut r = regla(5, "externa", "gateway", Accion::Permitir);
        r.nombre = "Web pública".into();
        r.puertos = Puertos::Lista(vec![RangoPuerto::uno(80), RangoPuerto { desde: 8000, hasta: 8100 }]);
        r.origen = Direccion::Ip { red: "198.51.100.0/24".parse().unwrap() };
        r.log = true;
        c.reglas.push(r);
        let t = generado(&c);
        assert!(t.contains("iifname { \"eth0\" } ip saddr 198.51.100.0/24 tcp dport { 80, 8000-8100 } counter log prefix \"netctl r5 \" accept comment \"r5_Web_p_blica\""), "{t}");
    }

    #[test]
    fn regla_inactiva_fuera() {
        let mut c = router();
        let mut r = regla(5, "externa", "gateway", Accion::Permitir);
        r.activa = false;
        c.reglas.push(r);
        assert!(!generado(&c).contains("comment \"r5_"));
    }

    #[test]
    fn rechazar_y_tcpudp() {
        let mut c = router();
        let mut r = regla(2, "interna", "externa", Accion::Rechazar);
        r.protocolo = Protocolo::TcpUdp;
        r.puertos = Puertos::Lista(vec![RangoPuerto::uno(53)]);
        c.reglas.push(r);
        let t = generado(&c);
        assert!(t.contains("iifname { \"eth1\" } oifname { \"eth0\" } meta l4proto { tcp, udp } th dport 53 counter reject comment \"r2_"), "{t}");
    }

    #[test]
    fn grupos_como_sets() {
        let mut c = router();
        c.grupos_ip.push(GrupoIp {
            id: 3,
            nombre: "Oficina".into(),
            miembros: vec![
                Direccion::Ip { red: "198.51.100.0/24".parse().unwrap() },
                Direccion::Rango { desde: "203.0.113.10".parse().unwrap(), hasta: "203.0.113.20".parse().unwrap() },
                Direccion::Ip { red: "2001:db8::/32".parse().unwrap() },
            ],
        });
        c.grupos_puertos.push(GrupoPuertos { id: 4, nombre: "Web".into(), puertos: vec![RangoPuerto::uno(80), RangoPuerto::uno(443)] });
        let mut r = regla(1, "externa", "gateway", Accion::Permitir);
        r.origen = Direccion::Grupo { id: 3 };
        r.puertos = Puertos::Grupo(4);
        c.reglas.push(r);
        let t = generado(&c);
        assert!(t.contains("set gi3_4 { type ipv4_addr; flags interval; elements = { 198.51.100.0/24, 203.0.113.10-203.0.113.20 } }"), "{t}");
        assert!(t.contains("set gi3_6 { type ipv6_addr; flags interval; elements = { 2001:db8::/32 } }"));
        assert!(t.contains("set gp4 { type inet_service; flags interval; elements = { 80, 443 } }"));
        assert!(t.contains("ip saddr @gi3_4 tcp dport @gp4 counter accept"));
        assert!(t.contains("ip6 saddr @gi3_6 tcp dport @gp4 counter accept"));
    }

    #[test]
    fn reenvio_dnat() {
        let mut c = router();
        let mut r = reenvio(9, "192.168.1.20", 3389);
        r.nombre = "PC \"juegos\"".into();
        c.reenvios.push(r);
        let t = generado(&c);
        let pre = &t[pos(&t, "chain prerouting")..pos(&t, "chain postrouting")];
        assert!(pre.contains("iifname { \"eth0\" } tcp dport 3389 counter dnat ip to 192.168.1.20 comment \"f9_PC__juegos_\""), "{pre}");
        assert!(t.contains("ct status dnat accept"));
    }

    #[test]
    fn reenvio_puerto_distinto() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 2222);
        r.interno = Some(22);
        c.reenvios.push(r);
        assert!(generado(&c).contains("tcp dport 2222 counter dnat ip to 192.168.1.20:22"));
    }

    #[test]
    fn reenvio_rango() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 27015);
        r.protocolo = Protocolo::TcpUdp;
        r.externo = vec![RangoPuerto { desde: 27015, hasta: 27030 }];
        c.reenvios.push(r);
        assert!(generado(&c).contains("meta l4proto { tcp, udp } th dport 27015-27030 counter dnat ip to 192.168.1.20 "));
    }

    #[test]
    fn reenvio_desde_y_wan() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 80);
        r.desde = vec![Direccion::Ip { red: "198.51.100.7/32".parse().unwrap() }];
        r.wan = Some("eth0".into());
        c.reenvios.push(r);
        assert!(generado(&c).contains("iifname { \"eth0\" } ip saddr { 198.51.100.7/32 } tcp dport 80 counter dnat"));
    }

    #[test]
    fn reenvio_loopback() {
        let mut c = router();
        c.reenvios.push(reenvio(1, "192.168.1.20", 25565));
        let t = generado(&c);
        let pre = &t[pos(&t, "chain prerouting")..pos(&t, "chain postrouting")];
        assert!(pre.contains("iifname != { \"eth0\" } ip daddr { 203.0.113.4 } tcp dport 25565 counter dnat ip to 192.168.1.20"), "{pre}");
        let post = &t[pos(&t, "chain postrouting")..];
        assert!(post.contains("iifname != { \"eth0\" } ip daddr 192.168.1.20 tcp dport 25565 ct status dnat masquerade"), "{post}");
        c.reenvios[0].loopback = false;
        assert!(!generado(&c).contains("iifname != { \"eth0\" } ip daddr"));
    }

    #[test]
    fn reenvio_caducado_o_desactivado() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 80);
        r.caduca = Some(999);
        c.reenvios.push(r);
        assert!(!generado(&c).contains("dnat ip to"));
        c.reenvios[0].caduca = Some(1001);
        assert!(generado(&c).contains("dnat ip to"));
        c.reenvios[0].activo = false;
        assert!(!generado(&c).contains("dnat ip to"));
    }

    #[test]
    fn nat_masquerade_por_zona() {
        let mut c = router();
        let t = generado(&c);
        assert!(t.contains("iifname { \"eth1\" } oifname { \"eth0\" } masquerade"), "{t}");
        assert!(t.contains("iifname { \"wg0\" } oifname { \"eth0\" } masquerade"));
        c.nat.insert("interna".into(), false);
        assert!(!generado(&c).contains("iifname { \"eth1\" } oifname { \"eth0\" } masquerade"));
    }

    #[test]
    fn equipo_usa_su_ip() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 80);
        r.equipo = Direccion::Equipo { mac: "aa:bb:cc:dd:ee:ff".into(), ip: "192.168.1.33".parse().unwrap(), nombre: "NAS".into() };
        c.reenvios.push(r);
        assert!(generado(&c).contains("dnat ip to 192.168.1.33"));
    }

    #[test]
    fn errores_no_generan() {
        let mut c = router();
        c.reenvios.push(reenvio(1, "192.168.1.20", 8443));
        assert!(generar(&c, &fijas(), 0).is_err());
    }

    mod propiedades {
        use super::*;
        use proptest::prelude::*;

        fn accion() -> impl Strategy<Value = Accion> {
            prop_oneof![Just(Accion::Permitir), Just(Accion::Bloquear), Just(Accion::Rechazar)]
        }
        fn zona() -> impl Strategy<Value = &'static str> {
            prop_oneof![Just("externa"), Just("interna"), Just("gateway"), Just("vpn"), Just("dmz")]
        }

        proptest! {
            #[test]
            fn fijas_siempre(
                reglas in proptest::collection::vec((zona(), zona(), accion(), 1u16..65535, "[ -~]{0,20}"), 0..8),
                bloquear in proptest::collection::vec((zona(), zona()), 0..10),
                internet in any::<bool>(),
            ) {
                let mut c = router();
                for (i, (o, d, a, p, n)) in reglas.into_iter().enumerate() {
                    let mut r = regla(i as u64 + 1, o, d, a);
                    r.nombre = n;
                    r.puertos = Puertos::Lista(vec![RangoPuerto::uno(p)]);
                    c.reglas.push(r);
                }
                for (o, d) in bloquear {
                    c.politicas.push(PoliticaPar { origen: o.into(), destino: d.into(), politica: Politica::Bloquear });
                }
                let mut f = fijas();
                f.alcance_internet = internet;
                let t = generar(&c, &f, 0).unwrap();
                let input = &t[t.find("chain input").unwrap()..t.find("chain forward").unwrap()];
                let panel = input.find("tcp dport 8443 accept comment \"fija_panel\"").unwrap();
                let ssh = input.find("comment \"fija_ssh\"").unwrap();
                let primera_usuario = input.find("comment \"r").unwrap_or(usize::MAX);
                let primera_pol = input.find("comment \"p_").unwrap();
                prop_assert!(panel < primera_pol && ssh < primera_pol && panel < primera_usuario);
                // ningún comentario rompe las comillas
                for l in t.lines().filter(|l| l.contains("comment")) {
                    prop_assert_eq!(l.matches('"').count() % 2, 0);
                    prop_assert!(!l.contains(';') || l.contains("policy"));
                }
            }
        }
    }
}
