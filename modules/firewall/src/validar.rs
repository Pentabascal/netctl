//! Validación del modelo con mensajes en castellano llano que dicen cómo arreglarlo.

use std::net::IpAddr;

use crate::modelo::*;

fn solapan(a: Protocolo, b: Protocolo) -> bool {
    (a.incluye_tcp() && b.incluye_tcp()) || (a.incluye_udp() && b.incluye_udp())
}

/// Puertos que no se pueden usar como «de fuera» en un reenvío (los protegen las reglas fijas).
pub fn puertos_reservados(f: &Fijas) -> Vec<(Protocolo, u16, String)> {
    let mut v = vec![
        (Protocolo::Tcp, f.panel, "el panel de netctl".to_string()),
        (Protocolo::Tcp, f.ssh, "SSH (acceso remoto a esta máquina)".to_string()),
    ];
    for (i, p) in &f.wg {
        v.push((Protocolo::Udp, *p, format!("WireGuard ({i})")));
    }
    v
}

/// Lee `80,443, 8000-8100` → lista de rangos. Error en castellano.
pub fn parse_puertos(s: &str) -> Result<Vec<RangoPuerto>, String> {
    fn uno(x: &str) -> Result<u16, String> {
        match x.trim().parse::<u16>() {
            Ok(p) if p >= 1 => Ok(p),
            _ => Err(format!("«{}» no es un puerto válido: usa números del 1 al 65535.", x.trim())),
        }
    }
    let mut v = vec![];
    for parte in s.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        let r = match parte.split_once('-') {
            Some((a, b)) => {
                let (a, b) = (uno(a)?, uno(b)?);
                if a > b {
                    return Err(format!(
                        "El rango {parte} está al revés: escribe primero el número menor ({b}-{a})."
                    ));
                }
                RangoPuerto { desde: a, hasta: b }
            }
            None => RangoPuerto::uno(uno(parte)?),
        };
        v.push(r);
    }
    if v.is_empty() {
        return Err("Escribe al menos un puerto, por ejemplo 80 o 8000-8100.".into());
    }
    Ok(v)
}

fn rango_valido(r: &RangoPuerto) -> bool {
    r.desde >= 1 && r.desde <= r.hasta
}

/// IPv4 de destino de un reenvío (equipo o IP sola).
pub fn ip_reenvio(r: &Reenvio) -> Option<std::net::Ipv4Addr> {
    match &r.equipo {
        Direccion::Equipo { ip: IpAddr::V4(ip), .. } => Some(*ip),
        Direccion::Ip { red: ipnet::IpNet::V4(n) } if n.prefix_len() == 32 => Some(n.addr()),
        _ => None,
    }
}

fn ocupado(c: &Config, f: &Fijas, proto: Protocolo, p: u16, ignorar: Option<u64>) -> bool {
    puertos_reservados(f).iter().any(|(pr, rp, _)| *rp == p && solapan(*pr, proto))
        || c.reenvios.iter().any(|o| {
            o.activo
                && Some(o.id) != ignorar
                && solapan(o.protocolo, proto)
                && o.externo.iter().any(|r| r.contiene(p))
        })
}

/// Comprueba un reenvío contra los demás y las reglas fijas. Devuelve errores (impiden guardar).
pub fn choques_reenvio(c: &Config, f: &Fijas, r: &Reenvio) -> Vec<String> {
    let mut e = vec![];
    if !r.protocolo.usa_puertos() {
        e.push("Elige TCP, UDP o los dos: los reenvíos funcionan por puerto.".into());
        return e;
    }
    if r.externo.is_empty() {
        e.push("Elige al menos un puerto.".into());
    }
    if r.externo.iter().any(|x| !rango_valido(x)) {
        e.push("Algún puerto no es válido: usa números del 1 al 65535 y rangos de menor a mayor.".into());
        return e;
    }
    if r.interno.is_some()
        && (r.externo.len() != 1 || r.externo[0].desde != r.externo[0].hasta)
    {
        e.push("Un puerto de dentro distinto solo se puede usar con un único puerto de fuera, no con varios ni con un rango.".into());
    }
    if r.interno == Some(0) {
        e.push("El puerto de dentro tiene que estar entre 1 y 65535.".into());
    }
    if ip_reenvio(r).is_none() {
        e.push("Elige un único equipo con dirección IPv4 (los reenvíos de puertos son solo para IPv4).".into());
    }
    if let Some(w) = &r.wan
        && !c.ifaces_externas().contains(w)
    {
        e.push(format!("La interfaz «{w}» no está en la zona Externa: elige otra o «Todas»."));
    }
    for d in &r.desde {
        if let Err(x) = validar_dir(c, d) {
            e.push(x);
        }
    }
    for rango in &r.externo {
        for (pr, p, nombre) in puertos_reservados(f) {
            if solapan(pr, r.protocolo) && rango.contiene(p) {
                let sig = siguiente_libre(c, f, r.protocolo, p, Some(r.id))
                    .map(|s| format!(", por ejemplo {s}"))
                    .unwrap_or_default();
                e.push(format!("El puerto {p} lo usa {nombre}. Elige otro puerto de fuera{sig}."));
            }
        }
        for o in c.reenvios.iter().filter(|o| o.activo && o.id != r.id && solapan(o.protocolo, r.protocolo)) {
            if let Some(x) = o.externo.iter().find(|x| x.solapa(rango)) {
                let comun = rango.desde.max(x.desde);
                let sig = siguiente_libre(c, f, r.protocolo, comun, Some(r.id))
                    .map(|s| format!(", por ejemplo {s}"))
                    .unwrap_or_default();
                e.push(format!(
                    "El puerto {comun} ya lo usa el reenvío «{}». Elige otro puerto de fuera{sig}.",
                    o.nombre
                ));
            }
        }
    }
    e
}

/// Primer puerto ≥ `desde` libre para `proto` (ni reenvíos ni fijas). `ignorar`: id del reenvío
/// que se está editando.
pub fn siguiente_libre(c: &Config, f: &Fijas, proto: Protocolo, desde: u16, ignorar: Option<u64>) -> Option<u16> {
    (desde.max(1)..=u16::MAX).find(|p| !ocupado(c, f, proto, *p, ignorar))
}

/// Avisos (no impiden guardar): servicio delicado abierto a todos, destino en VPN/Gateway.
pub fn avisos_reenvio(c: &Config, r: &Reenvio) -> Vec<String> {
    let mut a = vec![];
    let mut puertos = r.externo.clone();
    if let Some(i) = r.interno {
        puertos.push(RangoPuerto::uno(i));
    }
    if r.desde.is_empty()
        && let Some(nombre) = crate::plantillas::puerto_delicado(&puertos)
    {
        a.push(format!(
            "Abrir {nombre} a todo internet es peligroso: lo atacan robots a diario. Mejor limítalo con «Solo desde estas IPs» o entra por la VPN."
        ));
    }
    if let Some(ip) = ip_reenvio(r) {
        let ip = IpAddr::V4(ip);
        let vpn = c.zonas_tipo(TipoZona::Vpn);
        for (iface, redes) in &c.direcciones {
            for red in redes {
                if red.addr() == ip {
                    a.push("Ese equipo es esta máquina: no hace falta reenvío. Abre el puerto con una regla de Externa → Esta máquina.".into());
                } else if red.contains(&ip)
                    && c.interfaces.get(iface).is_some_and(|z| vpn.contains(z))
                {
                    a.push("Ese equipo está en la VPN: no hace falta reenvío, los clientes VPN ya se ven entre sí y con la red según sus permisos en WireGuard.".into());
                }
            }
        }
    }
    a
}

/// ¿En qué reglas y reenvíos se usa el grupo de IPs o de puertos `id`?
pub fn usos_grupo(c: &Config, id: u64) -> Vec<String> {
    let g = |d: &Direccion| matches!(d, Direccion::Grupo { id: x } if *x == id);
    let mut v = vec![];
    for r in &c.reglas {
        if g(&r.origen) || g(&r.destino) || r.puertos == Puertos::Grupo(id) {
            v.push(format!("Regla «{}»", r.nombre));
        }
    }
    for r in &c.reenvios {
        if r.desde.iter().any(g) {
            v.push(format!("Reenvío «{}»", r.nombre));
        }
    }
    v
}

/// Familia de una dirección: Some(true)=IPv4, Some(false)=IPv6, None=las dos o cualquiera.
fn familia(c: &Config, d: &Direccion) -> Option<bool> {
    match d {
        Direccion::Cualquiera | Direccion::Interfaz { .. } | Direccion::Mac { .. } => None,
        Direccion::Equipo { ip, .. } => Some(ip.is_ipv4()),
        Direccion::Ip { red } => Some(red.addr().is_ipv4()),
        Direccion::Rango { desde, .. } => Some(desde.is_ipv4()),
        Direccion::Grupo { id } => {
            let g = c.grupos_ip.iter().find(|g| g.id == *id)?;
            // Los grupos no se anidan (lo rechaza `validar`): un miembro grupo no cuenta, y así
            // un grupo que se contiene a sí mismo no recurre sin fin.
            let f: Vec<_> = g
                .miembros
                .iter()
                .filter(|m| !matches!(m, Direccion::Grupo { .. }))
                .filter_map(|m| familia(c, m))
                .collect();
            if !f.is_empty() && f.iter().all(|x| *x == f[0]) { Some(f[0]) } else { None }
        }
    }
}

pub fn validar_dir(c: &Config, d: &Direccion) -> Result<(), String> {
    match d {
        Direccion::Cualquiera | Direccion::Ip { .. } => Ok(()),
        Direccion::Equipo { mac, nombre, .. } => {
            if es_mac(mac) {
                Ok(())
            } else {
                Err(format!("El equipo «{nombre}» tiene una MAC no válida ({mac}). Vuelve a elegirlo en la lista."))
            }
        }
        Direccion::Rango { desde, hasta } => {
            if !misma_familia(*desde, *hasta) {
                Err(format!("El rango {desde}-{hasta} mezcla IPv4 e IPv6: usa dos del mismo tipo."))
            } else if desde > hasta {
                Err(format!("El rango {desde}-{hasta} está al revés: escribe primero la IP menor."))
            } else {
                Ok(())
            }
        }
        Direccion::Grupo { id } => {
            if c.grupos_ip.iter().any(|g| g.id == *id) {
                Ok(())
            } else {
                Err("Se usa un grupo de IPs que ya no existe: elige otro.".into())
            }
        }
        Direccion::Interfaz { iface } => {
            if es_iface(iface) && c.interfaces.contains_key(iface) {
                Ok(())
            } else {
                Err(format!("La tarjeta «{}» ya no existe: elige otra.", crate::nombre_seguro(iface)))
            }
        }
        Direccion::Mac { mac } => {
            if es_mac(mac) {
                Ok(())
            } else {
                Err(format!("La MAC «{}» no es válida: escríbela como aa:bb:cc:dd:ee:ff.", crate::nombre_seguro(mac)))
            }
        }
    }
}

/// Hora `HH:MM` de 00:00 a 23:59.
pub fn es_hora(h: &str) -> bool {
    let b = h.as_bytes();
    b.len() == 5
        && b[2] == b':'
        && h[..2].parse::<u8>().is_ok_and(|x| x < 24 && b[0].is_ascii_digit() && b[1].is_ascii_digit())
        && h[3..].parse::<u8>().is_ok_and(|x| x < 60 && b[3].is_ascii_digit() && b[4].is_ascii_digit())
}

fn errores_franja(f: &Franja, e: &mut Vec<String>) {
    for h in [&f.desde, &f.hasta] {
        if !es_hora(h) {
            e.push(format!("La hora «{}» no es válida: usa HH:MM, por ejemplo 08:30.", h.chars().take(12).collect::<String>()));
        }
    }
}

fn errores_dias(d: &[u8], e: &mut Vec<String>) {
    if d.is_empty() {
        e.push("Elige al menos un día.".into());
    } else if d.iter().any(|x| *x > 6) {
        e.push("Los días van de lunes (0) a domingo (6).".into());
    }
}

/// Errores del horario de una regla (sin el prefijo «Regla …»).
pub fn errores_horario(h: &Horario) -> Vec<String> {
    let mut e = vec![];
    match h {
        Horario::Siempre => {}
        Horario::Diario(f) => errores_franja(f, &mut e),
        Horario::Semanal { dias, franja } => {
            errores_dias(dias, &mut e);
            errores_franja(franja, &mut e);
        }
        Horario::UnaVez { inicio, fin } => {
            if fin <= inicio {
                e.push("El fin tiene que ser después del inicio.".into());
            }
        }
        Horario::AMedida(tramos) => {
            if tramos.is_empty() {
                e.push("Añade al menos una franja.".into());
            }
            for t in tramos {
                errores_dias(&t.dias, &mut e);
                errores_franja(&t.franja, &mut e);
            }
        }
    }
    e
}

/// Errores de lo nuevo de la fase 2.5 (red/MAC, versión IP, horario), con el prefijo de la regla.
fn errores_regla_25(c: &Config, r: &Regla) -> Vec<String> {
    let mut e = vec![];
    if matches!(r.destino, Direccion::Mac { .. }) {
        e.push("La MAC solo se puede usar como origen.".to_string());
    }
    for (d, zona) in [(&r.origen, &r.origen_zona), (&r.destino, &r.destino_zona)] {
        if let Direccion::Interfaz { iface } = d
            && c.interfaces.get(iface).is_some_and(|z| z != zona)
        {
            let nombre = c.zona(zona).map(|z| z.nombre.as_str()).unwrap_or(zona);
            e.push(format!("La tarjeta {} no es de la zona {nombre}.", crate::nombre_seguro(iface)));
        }
    }
    let quiere = match r.version_ip {
        VersionIp::Ambas => None,
        VersionIp::V4 => Some(true),
        VersionIp::V6 => Some(false),
    };
    if let Some(q) = quiere {
        for d in [&r.origen, &r.destino] {
            if let Some(f) = familia(c, d)
                && f != q
            {
                e.push(if q { "Una IPv6 no casa con «Solo IPv4».".to_string() } else { "Una IPv4 no casa con «Solo IPv6».".to_string() });
                break;
            }
        }
    }
    if r.estado == EstadoConexion::Elegir(vec![]) {
        e.push("Elige al menos un estado de conexión.".to_string());
    }
    e.extend(errores_horario(&r.horario));
    let n = &r.nombre;
    e.into_iter().map(|x| format!("Regla «{n}»: {x}")).collect()
}

/// Todos los errores de la configuración completa.
pub fn validar(c: &Config, f: &Fijas) -> Vec<String> {
    let mut e = vec![];
    let existe = |z: &str| c.zona(z).is_some();
    let mut vistas = std::collections::BTreeSet::new();
    for z in &c.zonas {
        if !es_id_zona(&z.id) {
            e.push(format!("La zona «{}» tiene un identificador no válido.", z.nombre));
        }
        if !vistas.insert(z.id.as_str()) {
            e.push(format!("Hay dos zonas con el identificador «{}».", z.id));
        }
        if z.nombre.trim().is_empty() {
            e.push("Hay una zona sin nombre: ponle uno.".into());
        }
    }
    for (i, z) in &c.interfaces {
        if !es_iface(i) {
            e.push(format!("«{}» no es un nombre de interfaz válido.", crate::nombre_seguro(i)));
        }
        match c.zona(z) {
            None => e.push(format!("La interfaz {} está en una zona que no existe: asígnale otra.", crate::nombre_seguro(i))),
            Some(zz) if zz.tipo == TipoZona::Gateway => {
                e.push(format!("La zona «Esta máquina» no lleva interfaces: asigna {} a otra.", crate::nombre_seguro(i)))
            }
            _ => {}
        }
    }
    for p in &c.politicas {
        if !existe(&p.origen) || !existe(&p.destino) {
            e.push("Hay una política entre zonas que ya no existen.".into());
        }
    }
    for r in &c.reglas {
        let n = &r.nombre;
        if !existe(&r.origen_zona) || !existe(&r.destino_zona) {
            e.push(format!("La regla «{n}» usa una zona que ya no existe."));
        }
        if !r.protocolo.usa_puertos() && r.puertos != Puertos::Cualquiera {
            e.push(format!("La regla «{n}» tiene puertos, pero el protocolo elegido no usa puertos: elige TCP, UDP o los dos."));
        }
        match &r.puertos {
            Puertos::Lista(l) if l.is_empty() || l.iter().any(|x| !rango_valido(x)) => {
                e.push(format!("La regla «{n}» tiene puertos no válidos: usa números del 1 al 65535."))
            }
            Puertos::Grupo(id) if !c.grupos_puertos.iter().any(|g| g.id == *id) => {
                e.push(format!("La regla «{n}» usa un grupo de puertos que ya no existe."))
            }
            _ => {}
        }
        if !r.protocolo.usa_puertos() && r.puertos_origen != Puertos::Cualquiera {
            e.push(format!("La regla «{n}» tiene puertos de origen, pero el protocolo elegido no usa puertos: elige TCP, UDP o los dos."));
        }
        match &r.puertos_origen {
            Puertos::Lista(l) if l.is_empty() || l.iter().any(|x| !rango_valido(x)) => {
                e.push(format!("La regla «{n}» tiene puertos de origen no válidos: usa números del 1 al 65535."))
            }
            Puertos::Grupo(id) if !c.grupos_puertos.iter().any(|g| g.id == *id) => {
                e.push(format!("La regla «{n}» usa un grupo de puertos que ya no existe."))
            }
            _ => {}
        }
        for d in [&r.origen, &r.destino] {
            if let Err(x) = validar_dir(c, d) {
                e.push(format!("Regla «{n}»: {x}"));
            }
        }
        e.extend(errores_regla_25(c, r));
        if let (Some(a), Some(b)) = (familia(c, &r.origen), familia(c, &r.destino))
            && a != b
        {
            e.push(format!("La regla «{n}» mezcla IPv4 e IPv6 en origen y destino: usa el mismo tipo en los dos."));
        }
    }
    for g in &c.grupos_ip {
        for m in &g.miembros {
            if matches!(m, Direccion::Grupo { .. } | Direccion::Cualquiera | Direccion::Interfaz { .. } | Direccion::Mac { .. }) {
                e.push(format!("El grupo «{}» solo puede tener equipos, IPs, redes o rangos.", g.nombre));
            } else if let Err(x) = validar_dir(c, m) {
                e.push(format!("Grupo «{}»: {x}", g.nombre));
            }
        }
    }
    for g in &c.grupos_puertos {
        if g.puertos.iter().any(|x| !rango_valido(x)) {
            e.push(format!("El grupo de puertos «{}» tiene puertos no válidos.", g.nombre));
        }
    }
    for r in c.reenvios.iter().filter(|r| r.activo) {
        e.extend(choques_reenvio(c, f, r));
    }
    e
}

pub fn es_mac(s: &str) -> bool {
    let p: Vec<_> = s.split(':').collect();
    p.len() == 6 && p.iter().all(|x| x.len() == 2 && x.chars().all(|c| c.is_ascii_hexdigit()))
}

pub fn es_iface(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 15
        && s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'))
}

pub fn es_id_zona(s: &str) -> bool {
    !s.is_empty() && s.len() <= 20 && s.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn misma_familia(a: IpAddr, b: IpAddr) -> bool {
    a.is_ipv4() == b.is_ipv4()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::*;

    fn errores_de(r: Regla) -> Vec<String> {
        let mut c = router();
        c.reglas.push(r);
        validar(&c, &fijas())
    }

    #[test]
    fn mac_solo_origen() {
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.destino = Direccion::Mac { mac: "aa:bb:cc:dd:ee:ff".into() };
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: La MAC solo se puede usar como origen.".to_string()]);
        r.destino = Direccion::Cualquiera;
        r.origen = Direccion::Mac { mac: "aa:bb:cc:dd:ee:ff".into() };
        assert!(errores_de(r.clone()).is_empty());
        r.origen = Direccion::Mac { mac: "nada".into() };
        assert_eq!(errores_de(r).len(), 1);
    }

    #[test]
    fn red_de_otra_zona() {
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.origen = Direccion::Interfaz { iface: "eth0".into() };
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: La tarjeta eth0 no es de la zona Interna.".to_string()]);
        r.origen = Direccion::Interfaz { iface: "eth1".into() };
        r.destino = Direccion::Interfaz { iface: "eth0".into() };
        assert!(errores_de(r).is_empty());
    }

    #[test]
    fn hora_invalida() {
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.horario = Horario::Diario(Franja { desde: "25:00".into(), hasta: "17:00".into() });
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: La hora «25:00» no es válida: usa HH:MM, por ejemplo 08:30.".to_string()]);
        r.horario = Horario::Semanal { dias: vec![], franja: Franja { desde: "08:00".into(), hasta: "17:00".into() } };
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: Elige al menos un día.".to_string()]);
        r.horario = Horario::Semanal { dias: vec![0, 7], franja: Franja { desde: "08:00".into(), hasta: "17:00".into() } };
        assert_eq!(errores_de(r.clone()).len(), 1);
        r.horario = Horario::UnaVez { inicio: 2000, fin: 1000 };
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: El fin tiene que ser después del inicio.".to_string()]);
        r.horario = Horario::AMedida(vec![Tramo { dias: vec![5, 6], franja: Franja { desde: "22:00".into(), hasta: "06:00".into() } }]);
        assert!(errores_de(r).is_empty());
    }

    #[test]
    fn version_ip_choca() {
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.version_ip = VersionIp::V4;
        r.origen = Direccion::Ip { red: "fd00::/64".parse().unwrap() };
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: Una IPv6 no casa con «Solo IPv4».".to_string()]);
        r.version_ip = VersionIp::V6;
        r.origen = Direccion::Ip { red: "192.168.1.0/24".parse().unwrap() };
        assert_eq!(errores_de(r.clone()), vec!["Regla «regla 1»: Una IPv4 no casa con «Solo IPv6».".to_string()]);
        r.version_ip = VersionIp::Ambas;
        assert!(errores_de(r).is_empty());
    }

    #[test]
    fn estado_elegir_vacio() {
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.estado = EstadoConexion::Elegir(vec![]);
        assert_eq!(errores_de(r), vec!["Regla «regla 1»: Elige al menos un estado de conexión.".to_string()]);
    }

    #[test]
    fn protocolo_otro_y_puertos() {
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.protocolo = Protocolo::Otro;
        r.puertos = Puertos::Cualquiera;
        for n in [0u8, 47, 255] {
            r.numero_protocolo = n;
            assert!(errores_de(r.clone()).is_empty());
        }
        r.puertos_origen = Puertos::Lista(vec![RangoPuerto::uno(53)]);
        assert_eq!(errores_de(r).len(), 1);
    }

    #[test]
    fn puertos_texto() {
        assert_eq!(parse_puertos("80, 443,8000-8100").unwrap(), vec![RangoPuerto::uno(80), RangoPuerto::uno(443), RangoPuerto { desde: 8000, hasta: 8100 }]);
        assert!(parse_puertos("0").is_err());
        assert!(parse_puertos("70000").is_err());
        assert!(parse_puertos("90-80").unwrap_err().contains("al revés"));
        assert!(parse_puertos("abc").is_err());
        assert!(parse_puertos("").is_err());
    }

    #[test]
    fn choque_panel() {
        let c = router();
        let e = choques_reenvio(&c, &fijas(), &reenvio(1, "192.168.1.20", 8443));
        assert_eq!(e.len(), 1);
        assert!(e[0].contains("panel") && e[0].contains("8444"), "{e:?}");
        // UDP 8443 no choca con el panel (TCP)
        let mut r = reenvio(1, "192.168.1.20", 8443);
        r.protocolo = Protocolo::Udp;
        assert!(choques_reenvio(&c, &fijas(), &r).is_empty());
        // WireGuard es UDP
        let mut r = reenvio(1, "192.168.1.20", 51820);
        r.protocolo = Protocolo::TcpUdp;
        assert!(choques_reenvio(&c, &fijas(), &r)[0].contains("WireGuard"));
    }

    #[test]
    fn choque_entre_reenvios() {
        let mut c = router();
        let mut a = reenvio(1, "192.168.1.20", 27015);
        a.externo = vec![RangoPuerto { desde: 27015, hasta: 27030 }];
        c.reenvios.push(a);
        let e = choques_reenvio(&c, &fijas(), &reenvio(2, "192.168.1.30", 27020));
        assert!(e[0].contains("r1") && e[0].contains("27031"), "{e:?}");
        // editar el propio reenvío no choca consigo mismo
        let mut mismo = c.reenvios[0].clone();
        mismo.nombre = "otro".into();
        assert!(choques_reenvio(&c, &fijas(), &mismo).is_empty());
        // desactivado no choca
        c.reenvios[0].activo = false;
        assert!(choques_reenvio(&c, &fijas(), &reenvio(2, "192.168.1.30", 27020)).is_empty());
    }

    #[test]
    fn siguiente_libre_salta_ocupados() {
        let mut c = router();
        c.reenvios.push(reenvio(1, "192.168.1.20", 8444));
        assert_eq!(siguiente_libre(&c, &fijas(), Protocolo::Tcp, 8443, None), Some(8445));
        assert_eq!(siguiente_libre(&c, &fijas(), Protocolo::Udp, 8443, None), Some(8443));
    }

    #[test]
    fn rango_con_puerto_interno_error() {
        let c = router();
        let mut r = reenvio(1, "192.168.1.20", 27015);
        r.externo = vec![RangoPuerto { desde: 27015, hasta: 27030 }];
        r.interno = Some(27015);
        assert!(!choques_reenvio(&c, &fijas(), &r).is_empty());
    }

    #[test]
    fn avisos() {
        let c = router();
        let a = avisos_reenvio(&c, &reenvio(1, "192.168.1.20", 3389));
        assert!(a.iter().any(|x| x.contains("Escritorio remoto")), "{a:?}");
        let mut r = reenvio(1, "192.168.1.20", 3389);
        r.desde = vec![Direccion::Ip { red: "198.51.100.7/32".parse().unwrap() }];
        assert!(avisos_reenvio(&c, &r).is_empty());
        let a = avisos_reenvio(&c, &reenvio(1, "10.8.0.5", 80));
        assert!(a.iter().any(|x| x.contains("VPN")), "{a:?}");
        let a = avisos_reenvio(&c, &reenvio(1, "192.168.1.1", 80));
        assert!(a.iter().any(|x| x.contains("esta máquina")), "{a:?}");
    }

    #[test]
    fn grupo_en_uso() {
        let mut c = router();
        c.grupos_ip.push(GrupoIp { id: 7, nombre: "Casa".into(), miembros: vec![] });
        let mut r = regla(1, "externa", "gateway", Accion::Permitir);
        r.origen = Direccion::Grupo { id: 7 };
        c.reglas.push(r);
        let u = usos_grupo(&c, 7);
        assert_eq!(u.len(), 1);
        assert!(u[0].contains("regla 1"));
        assert!(usos_grupo(&c, 8).is_empty());
    }

    #[test]
    fn rango_ip() {
        let mut c = router();
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.origen = Direccion::Rango { desde: "192.168.1.10".parse().unwrap(), hasta: "192.168.1.20".parse().unwrap() };
        c.reglas.push(r.clone());
        assert!(validar(&c, &fijas()).is_empty());
        r.origen = Direccion::Rango { desde: "192.168.1.20".parse().unwrap(), hasta: "192.168.1.10".parse().unwrap() };
        c.reglas[0] = r.clone();
        assert!(validar(&c, &fijas()).iter().any(|e| e.contains("al revés")));
        r.origen = Direccion::Rango { desde: "192.168.1.1".parse().unwrap(), hasta: "fe80::1".parse().unwrap() };
        c.reglas[0] = r;
        assert!(!validar(&c, &fijas()).is_empty());
    }

    #[test]
    fn grupo_dentro_de_si_mismo_no_revienta() {
        let mut c = router();
        c.grupos_ip.push(GrupoIp { id: 7, nombre: "Bucle".into(), miembros: vec![Direccion::Grupo { id: 7 }] });
        let mut r = regla(1, "externa", "gateway", Accion::Permitir);
        r.origen = Direccion::Grupo { id: 7 };
        r.destino = Direccion::Ip { red: "192.168.1.1/32".parse().unwrap() };
        c.reglas.push(r);
        let e = validar(&c, &fijas());
        assert!(e.iter().any(|x| x.contains("Bucle")), "{e:?}");
    }

    #[test]
    fn validar_detecta_basura() {
        let mut c = router();
        c.interfaces.insert("eth\"0".into(), "interna".into());
        assert!(!validar(&c, &fijas()).is_empty());
        let mut c = router();
        c.interfaces.insert("eth5".into(), "no_existe".into());
        assert!(!validar(&c, &fijas()).is_empty());
        let mut c = router();
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.protocolo = Protocolo::Icmp; // ICMP con puertos
        c.reglas.push(r);
        assert!(!validar(&c, &fijas()).is_empty());
        let mut c = router();
        let mut r = regla(1, "interna", "externa", Accion::Bloquear);
        r.origen = Direccion::Equipo { mac: "zz".into(), ip: "192.168.1.5".parse().unwrap(), nombre: "x".into() };
        c.reglas.push(r);
        assert!(!validar(&c, &fijas()).is_empty());
    }
}
