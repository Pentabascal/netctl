//! Modelo del proyecto y generación de los ficheros .conf de servidor y clientes.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::net::{IpAddr, Ipv4Addr};

use ipnet::{IpNet, Ipv4Net};
use serde::{Deserialize, Deserializer, Serialize};

use crate::claves;

const PRIVADAS: [&str; 3] = ["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"];

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Subred {
    pub id: u64,
    pub nombre: String,
    pub cidr: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Servidor {
    pub interfaz: String,
    pub endpoint: String,
    #[serde(deserialize_with = "texto_o_numero")]
    pub puerto: String,
    pub red_vpn: String,
    pub ip_servidor: String,
    pub iface_salida: String,
    pub dns: String,
    pub mtu: String,
    pub clave_privada: String,
    pub clave_publica: String,
    pub nat: bool,
    pub firewall: bool,
    pub entre_clientes: bool,
}

impl Default for Servidor {
    fn default() -> Self {
        let (privada, publica) = claves::generar_par();
        Self {
            interfaz: "wg0".into(),
            endpoint: "vpn.ejemplo.com".into(),
            puerto: "51820".into(),
            red_vpn: "10.8.0.0/24".into(),
            ip_servidor: "10.8.0.1".into(),
            iface_salida: "eth0".into(),
            dns: "1.1.1.1".into(),
            mtu: String::new(),
            clave_privada: privada,
            clave_publica: publica,
            nat: true,
            firewall: true,
            entre_clientes: false,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Cliente {
    pub nombre: String,
    pub ip: String,
    pub clave_privada: String,
    pub clave_publica: String,
    pub usar_psk: bool,
    pub psk: String,
    /// Todo el tráfico del cliente sale por la VPN (internet incluido).
    pub internet: bool,
    /// Acceso a todas las subredes de intranet definidas.
    pub toda_intranet: bool,
    /// IDs de subredes de intranet permitidas (si no es toda la intranet).
    pub subredes: Vec<u64>,
    /// CIDRs adicionales separados por comas.
    pub extra: String,
    pub usar_dns: bool,
    #[serde(deserialize_with = "texto_o_numero")]
    pub keepalive: String,
    /// Un cliente inactivo no sale en la config del servidor ni en las reglas.
    #[serde(default = "si")]
    pub activo: bool,
}

fn si() -> bool {
    true
}

impl Default for Cliente {
    fn default() -> Self {
        let (privada, publica) = claves::generar_par();
        Self {
            nombre: "cliente".into(),
            ip: String::new(),
            clave_privada: privada,
            clave_publica: publica,
            usar_psk: true,
            psk: claves::generar_psk(),
            internet: false,
            toda_intranet: true,
            subredes: Vec::new(),
            extra: String::new(),
            usar_dns: false,
            keepalive: "25".into(),
            activo: true,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, Default)]
#[serde(default)]
pub struct Proyecto {
    pub servidor: Servidor,
    pub intranet: Vec<Subred>,
    pub clientes: Vec<Cliente>,
    pub sig_id: u64,
}

/// Acepta un campo numérico como texto o como número (proyectos guardados con versiones anteriores).
fn texto_o_numero<'de, D: Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum V {
        T(String),
        N(u64),
    }
    Ok(match V::deserialize(d)? {
        V::T(t) => t,
        V::N(n) => n.to_string(),
    })
}

/// Comprueba un campo numérico escrito como texto: solo cifras y dentro de [min, max].
/// Si `vacio_ok`, un campo vacío es válido y devuelve `None`.
pub fn validar_numero(nombre: &str, t: &str, min: u32, max: u32, vacio_ok: bool) -> Result<Option<u32>, String> {
    let t = t.trim();
    if t.is_empty() {
        return if vacio_ok { Ok(None) } else { Err(format!("{nombre}: no puede estar vacío")) };
    }
    if !t.chars().all(|c| c.is_ascii_digit()) {
        return Err(format!("{nombre} «{t}»: solo puede contener números"));
    }
    match t.parse::<u32>() {
        Ok(n) if (min..=max).contains(&n) => Ok(Some(n)),
        _ => Err(format!("{nombre} «{t}»: debe estar entre {min} y {max}")),
    }
}

pub fn validar_puerto(t: &str) -> Result<Option<u32>, String> {
    validar_numero("Puerto", t, 1, 65535, false)
}

pub fn validar_mtu(t: &str) -> Result<Option<u32>, String> {
    validar_numero("MTU", t, 576, 9000, true)
}

pub fn validar_keepalive(t: &str) -> Result<Option<u32>, String> {
    validar_numero("Keepalive", t, 0, 65535, true)
}

/// Rechaza caracteres de control (saltos de línea, NUL, tabuladores…): un salto de línea en
/// cualquier campo que acabe en un `.conf` permitiría inyectar líneas como `PostUp = …`,
/// que `wg-quick` ejecuta como root.
pub fn sin_control(campo: &str, v: &str) -> Result<(), String> {
    if v.chars().any(char::is_control) {
        return Err(format!("{campo}: no puede contener caracteres de control (saltos de línea, etc.)"));
    }
    Ok(())
}

/// Nombre de interfaz de red: `^[A-Za-z0-9_.-]{1,15}$` y distinto de `.` y `..`.
/// Se usa en rutas (`/etc/wireguard/<iface>.conf`), reglas y unidades de systemd.
pub fn validar_interfaz(campo: &str, v: &str) -> Result<String, String> {
    let t = v.trim();
    let ok = (1..=15).contains(&t.len())
        && t.chars().all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        && t != "."
        && t != "..";
    if !ok || v.chars().any(char::is_control) {
        return Err(format!(
            "{campo} «{}» no es un nombre de interfaz válido (letras, números, «-», «_» o «.»; máximo 15)",
            t.escape_debug()
        ));
    }
    Ok(t.to_string())
}

/// Clave WireGuard: base64 de exactamente 32 bytes.
pub fn clave_valida(v: &str) -> bool {
    use base64::{Engine, engine::general_purpose::STANDARD};
    !v.chars().any(char::is_control) && STANDARD.decode(v.trim()).is_ok_and(|b| b.len() == 32)
}

/// Endpoint público: `host`, `host:puerto`, IPv6 o `[IPv6]:puerto`. Devuelve (host, puerto).
/// El host solo admite letras, números, `.` y `-` (nombre DNS) o una IP.
pub fn partes_endpoint(v: &str) -> Result<(String, Option<u32>), String> {
    let t = v.trim();
    let err = || format!("Endpoint «{}» no válido (ej. vpn.ejemplo.com o 203.0.113.4:51820)", t.escape_debug());
    if t.is_empty() || v.chars().any(char::is_control) {
        return Err(err());
    }
    if let Ok(ip) = t.parse::<std::net::Ipv6Addr>() {
        return Ok((ip.to_string(), None));
    }
    let (host, puerto) = if let Some(resto) = t.strip_prefix('[') {
        let (h, p) = resto.split_once("]:").ok_or_else(err)?;
        h.parse::<std::net::Ipv6Addr>().map_err(|_| err())?;
        (h, Some(p))
    } else {
        match t.split_once(':') {
            Some((h, p)) => (h, Some(p)),
            None => (t, None),
        }
    };
    let host_ok = host.parse::<std::net::Ipv6Addr>().is_ok()
        || (!host.is_empty()
            && host.len() <= 253
            && host.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
            && !host.starts_with(['-', '.']));
    if !host_ok {
        return Err(err());
    }
    let puerto = match puerto {
        None => None,
        Some(p) => Some(validar_numero("Puerto del endpoint", p, 1, 65535, false).map_err(|_| err())?.unwrap_or(0)),
    };
    Ok((host.to_string(), puerto))
}

/// Comprueba un valor justo antes de escribirlo en un `.conf` (defensa si se salta `validar`).
fn seguro<'a>(campo: &str, v: &'a str) -> Result<&'a str, String> {
    sin_control(campo, v)?;
    Ok(v)
}

/// Clave lista para el `.conf`: base64 de 32 bytes o error.
fn clave<'a>(campo: &str, v: &'a str) -> Result<&'a str, String> {
    if !clave_valida(v) {
        return Err(format!("{campo} no es válida"));
    }
    Ok(v.trim())
}

/// Normaliza un nombre para usarlo como fichero o comentario de iptables.
pub fn nombre_seguro(s: &str) -> String {
    let r: String = s
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    if r.is_empty() { "cliente".into() } else { r }
}

fn parse_lista(s: &str) -> Result<Vec<IpNet>, String> {
    s.split([',', ' ', '\n'])
        .map(str::trim)
        .filter(|x| !x.is_empty())
        .map(|x| {
            x.parse::<IpNet>()
                .or_else(|_| x.parse::<IpAddr>().map(IpNet::from))
                .map(|n| n.trunc())
                .map_err(|_| format!("«{x}» no es una red CIDR válida"))
        })
        .collect()
}

fn unir(v: &[IpNet]) -> String {
    v.iter().map(|n| n.to_string()).collect::<Vec<_>>().join(", ")
}

impl Proyecto {
    pub fn nuevo() -> Self {
        let mut p = Self::default();
        p.anadir_subred("LAN principal", "192.168.1.0/24");
        p
    }

    pub fn anadir_subred(&mut self, nombre: &str, cidr: &str) {
        self.sig_id += 1;
        self.intranet.push(Subred { id: self.sig_id, nombre: nombre.into(), cidr: cidr.into() });
    }

    pub fn red_vpn(&self) -> Result<Ipv4Net, String> {
        self.servidor
            .red_vpn
            .trim()
            .parse::<Ipv4Net>()
            .map(|n| n.trunc())
            .map_err(|_| format!("Red VPN «{}» no válida (ej. 10.8.0.0/24)", self.servidor.red_vpn))
    }

    /// Primera IP libre de la red VPN para un cliente nuevo.
    pub fn siguiente_ip(&self) -> Option<Ipv4Addr> {
        let red = self.red_vpn().ok()?;
        let usadas: HashSet<String> = self
            .clientes
            .iter()
            .map(|c| c.ip.trim().to_string())
            .chain(std::iter::once(self.servidor.ip_servidor.trim().to_string()))
            .collect();
        red.hosts().find(|h| !usadas.contains(&h.to_string()))
    }

    pub fn anadir_cliente(&mut self) -> usize {
        let mut c = Cliente::default();
        c.nombre = format!("cliente{}", self.clientes.len() + 1);
        c.ip = self.siguiente_ip().map(|i| i.to_string()).unwrap_or_default();
        self.clientes.push(c);
        self.clientes.len() - 1
    }

    pub(crate) fn dns(&self) -> Result<Vec<IpAddr>, String> {
        self.servidor
            .dns
            .split([',', ' '])
            .map(str::trim)
            .filter(|x| !x.is_empty())
            .map(|x| x.parse::<IpAddr>().map_err(|_| format!("DNS «{x}» no es una IP válida")))
            .collect()
    }

    /// Redes de destino (intranet + extra) a las que un cliente tiene acceso explícito.
    pub fn destinos(&self, c: &Cliente) -> Result<Vec<IpNet>, String> {
        let mut v = Vec::new();
        for s in &self.intranet {
            if c.toda_intranet || c.subredes.contains(&s.id) {
                v.extend(parse_lista(&s.cidr).map_err(|e| format!("Subred «{}»: {e}", s.nombre))?);
            }
        }
        v.extend(parse_lista(&c.extra).map_err(|e| format!("Cliente «{}»: {e}", c.nombre))?);
        Ok(IpNet::aggregate(&v))
    }

    fn todas_intranet(&self) -> Result<Vec<IpNet>, String> {
        let mut v = Vec::new();
        for s in &self.intranet {
            v.extend(parse_lista(&s.cidr).map_err(|e| format!("Subred «{}»: {e}", s.nombre))?);
        }
        Ok(v)
    }

    /// AllowedIPs del lado del cliente (qué rutas mete por el túnel).
    pub fn allowed_ips_cliente(&self, c: &Cliente) -> Result<String, String> {
        if c.internet {
            return Ok("0.0.0.0/0, ::/0".into());
        }
        let mut v = vec![IpNet::V4(self.red_vpn()?)];
        v.extend(self.destinos(c)?);
        Ok(unir(&IpNet::aggregate(&v)))
    }

    /// Comprueba todo el proyecto y devuelve la lista de errores.
    pub fn validar(&self) -> Vec<String> {
        let mut e = Vec::new();
        let s = &self.servidor;
        // Ningún texto que pueda llegar a un .conf o a un script de reglas lleva caracteres de
        // control (un salto de línea permitiría inyectar `PostUp = …`).
        for (campo, v) in [
            ("Interfaz", &s.interfaz),
            ("Endpoint", &s.endpoint),
            ("Puerto", &s.puerto),
            ("Red VPN", &s.red_vpn),
            ("IP del servidor", &s.ip_servidor),
            ("Interfaz de salida", &s.iface_salida),
            ("DNS", &s.dns),
            ("MTU", &s.mtu),
            ("Clave privada del servidor", &s.clave_privada),
            ("Clave pública del servidor", &s.clave_publica),
        ] {
            if let Err(x) = sin_control(campo, v) {
                e.push(x);
            }
        }
        if s.interfaz.trim().is_empty() {
            e.push("Falta el nombre de interfaz del servidor".into());
        } else if let Err(x) = validar_interfaz("Interfaz", &s.interfaz) {
            e.push(x);
        }
        if s.endpoint.trim().is_empty() {
            e.push("Falta el endpoint público del servidor (IP o dominio)".into());
        } else if let Err(x) = partes_endpoint(&s.endpoint) {
            e.push(x);
        }
        if let Err(x) = validar_puerto(&s.puerto) {
            e.push(x);
        }
        if s.iface_salida.trim().is_empty() {
            if s.nat || s.firewall {
                e.push("Falta la interfaz de salida (LAN/internet) del servidor".into());
            }
        } else if let Err(x) = validar_interfaz("Interfaz de salida", &s.iface_salida) {
            e.push(x);
        }
        if !clave_valida(&s.clave_privada) {
            e.push("La clave privada del servidor no es válida".into());
        }
        if !clave_valida(&s.clave_publica) {
            e.push("La clave pública del servidor no es válida".into());
        }
        for sub in &self.intranet {
            if let Err(x) = sin_control("Nombre de subred", &sub.nombre) {
                e.push(x);
            }
            if let Err(x) = sin_control(&format!("Subred «{}»", sub.nombre.escape_debug()), &sub.cidr) {
                e.push(x);
            }
        }
        if let Err(x) = self.dns() {
            e.push(x);
        }
        if let Err(x) = validar_mtu(&s.mtu) {
            e.push(x);
        }
        let red = match self.red_vpn() {
            Ok(r) => Some(r),
            Err(x) => {
                e.push(x);
                None
            }
        };
        let mut ips = HashSet::new();
        match s.ip_servidor.trim().parse::<Ipv4Addr>() {
            Ok(ip) => {
                if red.is_some_and(|r| !r.contains(&ip)) {
                    e.push(format!("La IP del servidor {ip} no está dentro de la red VPN"));
                }
                ips.insert(ip);
            }
            Err(_) => e.push("IP del servidor en la VPN no válida".into()),
        }
        if let Err(x) = self.todas_intranet() {
            e.push(x);
        }
        let mut nombres = HashSet::new();
        for c in &self.clientes {
            for (campo, v) in [
                ("Nombre del cliente", &c.nombre),
                ("IP", &c.ip),
                ("Clave privada", &c.clave_privada),
                ("Clave pública", &c.clave_publica),
                ("Clave precompartida", &c.psk),
                ("Redes extra", &c.extra),
                ("Keepalive", &c.keepalive),
            ] {
                if let Err(x) = sin_control(campo, v) {
                    e.push(format!("«{}»: {x}", c.nombre.escape_debug()));
                }
            }
            if !clave_valida(&c.clave_publica) {
                e.push(format!("«{}»: clave pública no válida", c.nombre.escape_debug()));
            }
            if !nombres.insert(nombre_seguro(&c.nombre)) {
                e.push(format!("Nombre de cliente repetido: «{}»", c.nombre));
            }
            match c.ip.trim().parse::<Ipv4Addr>() {
                Ok(ip) => {
                    if red.is_some_and(|r| !r.contains(&ip) || ip == r.network() || ip == r.broadcast()) {
                        e.push(format!("«{}»: la IP {ip} no es un host válido de la red VPN", c.nombre));
                    }
                    if !ips.insert(ip) {
                        e.push(format!("«{}»: la IP {ip} está repetida", c.nombre));
                    }
                }
                Err(_) => e.push(format!("«{}»: IP no válida", c.nombre)),
            }
            if claves::publica_de(&c.clave_privada).is_none() {
                e.push(format!("«{}»: clave privada no válida", c.nombre));
            }
            if c.usar_psk && claves::publica_de(&c.psk).is_none() {
                e.push(format!("«{}»: clave precompartida no válida", c.nombre));
            }
            if let Err(x) = validar_keepalive(&c.keepalive) {
                e.push(format!("«{}»: {x}", c.nombre));
            }
            if let Err(x) = parse_lista(&c.extra) {
                e.push(format!("«{}»: {x}", c.nombre));
            }
        }
        e
    }

    fn cadena(&self) -> String {
        // iptables limita los nombres de cadena a 28 caracteres
        let mut n = format!("{}-acl", nombre_seguro(&self.servidor.interfaz));
        n.truncate(28);
        n
    }

    /// Clientes que entran en la config del servidor y en las reglas.
    pub fn clientes_activos(&self) -> impl Iterator<Item = &Cliente> {
        self.clientes.iter().filter(|c| c.activo)
    }

    /// Redes privadas que se bloquean a un cliente con internet (intranet + RFC1918).
    pub(crate) fn privadas_bloqueadas(&self) -> Result<Vec<IpNet>, String> {
        let mut v = self.todas_intranet()?;
        v.extend(PRIVADAS.iter().map(|x| x.parse::<IpNet>().unwrap()));
        Ok(IpNet::aggregate(&v))
    }

    /// Reglas iptables (PostUp, PostDown).
    fn reglas(&self) -> Result<(Vec<String>, Vec<String>), String> {
        let s = &self.servidor;
        // Estas órdenes las ejecuta wg-quick como root en una shell: solo valores validados.
        let wg = validar_interfaz("Interfaz", &s.interfaz)?;
        let out = if s.nat || s.firewall {
            validar_interfaz("Interfaz de salida", &s.iface_salida)?
        } else {
            String::new()
        };
        let red = self.red_vpn()?;
        let ch = self.cadena();
        let mut up = vec!["sysctl -q -w net.ipv4.ip_forward=1".to_string()];
        let mut down = Vec::new();

        if s.firewall {
            up.push(format!("iptables -N {ch} 2>/dev/null; iptables -F {ch}"));
            up.push(format!("iptables -I FORWARD 1 -i {wg} -j {ch}"));
            up.push(format!(
                "iptables -I FORWARD 1 -o {wg} -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT"
            ));
            if s.entre_clientes {
                up.push(format!("iptables -A {ch} -s {red} -d {red} -j ACCEPT"));
            }
            let dns: Vec<IpAddr> = self.dns()?;
            for c in self.clientes_activos() {
                let ip: Ipv4Addr = c
                    .ip
                    .trim()
                    .parse()
                    .map_err(|_| format!("«{}»: IP no válida", c.nombre.escape_debug()))?;
                let com = format!("-m comment --comment {}", nombre_seguro(&c.nombre));
                let destinos = self.destinos(c)?;
                for d in &destinos {
                    up.push(format!("iptables -A {ch} -s {ip}/32 -d {d} {com} -j ACCEPT"));
                }
                if c.internet {
                    if c.usar_dns {
                        // Permite el DNS configurado aunque esté en una red privada
                        for d in dns.iter().filter(|d| d.is_ipv4()) {
                            for proto in ["udp", "tcp"] {
                                up.push(format!(
                                    "iptables -A {ch} -s {ip}/32 -d {d}/32 -p {proto} --dport 53 {com} -j ACCEPT"
                                ));
                            }
                        }
                    }
                    // Internet sí, pero redes privadas no permitidas: bloqueadas
                    for b in self.privadas_bloqueadas()? {
                        up.push(format!("iptables -A {ch} -s {ip}/32 -d {b} {com} -j DROP"));
                    }
                    up.push(format!("iptables -A {ch} -s {ip}/32 -o {out} {com} -j ACCEPT"));
                }
            }
            up.push(format!("iptables -A {ch} -j DROP"));

            down.push(format!("iptables -D FORWARD -i {wg} -j {ch}"));
            down.push(format!(
                "iptables -D FORWARD -o {wg} -m conntrack --ctstate RELATED,ESTABLISHED -j ACCEPT"
            ));
            down.push(format!("iptables -F {ch}; iptables -X {ch}"));
        } else {
            up.push(format!("iptables -I FORWARD 1 -i {wg} -j ACCEPT"));
            up.push(format!("iptables -I FORWARD 1 -o {wg} -j ACCEPT"));
            down.push(format!("iptables -D FORWARD -i {wg} -j ACCEPT"));
            down.push(format!("iptables -D FORWARD -o {wg} -j ACCEPT"));
        }
        if s.nat {
            up.push(format!("iptables -t nat -A POSTROUTING -s {red} -o {out} -j MASQUERADE"));
            down.push(format!("iptables -t nat -D POSTROUTING -s {red} -o {out} -j MASQUERADE"));
        }
        Ok((up, down))
    }

    /// Config del servidor con las reglas iptables en PostUp/PostDown (app de escritorio).
    pub fn config_servidor(&self) -> Result<String, String> {
        self.config_servidor_con(true)
    }

    /// Config del servidor para el panel: sin PostUp/PostDown (las reglas van en nftables).
    pub fn config_servidor_panel(&self) -> Result<String, String> {
        self.config_servidor_con(false)
    }

    fn config_servidor_con(&self, con_reglas: bool) -> Result<String, String> {
        let s = &self.servidor;
        let red = self.red_vpn()?;
        let iface = validar_interfaz("Interfaz", &s.interfaz)?;
        let ip_srv: Ipv4Addr =
            s.ip_servidor.trim().parse().map_err(|_| "IP del servidor en la VPN no válida".to_string())?;
        let puerto = validar_puerto(&s.puerto)?.unwrap_or_default();
        let mtu = validar_mtu(&s.mtu)?;
        let mut o = String::new();
        if con_reglas {
            let _ = writeln!(o, "# Servidor WireGuard — generado por wg-configurador");
            let _ = writeln!(o, "# Guardar como /etc/wireguard/{iface}.conf (chmod 600)");
            let _ = writeln!(o, "# Activar: systemctl enable --now wg-quick@{iface}\n");
        } else {
            let _ = writeln!(o, "# Servidor WireGuard — gestionado por netctl, no editar a mano\n");
        }
        let _ = writeln!(o, "[Interface]");
        let _ = writeln!(o, "Address = {ip_srv}/{}", red.prefix_len());
        let _ = writeln!(o, "ListenPort = {puerto}");
        let _ = writeln!(o, "PrivateKey = {}", clave("Clave privada del servidor", &s.clave_privada)?);
        if let Some(m) = mtu {
            let _ = writeln!(o, "MTU = {m}");
        }
        if con_reglas {
            let (up, down) = self.reglas()?;
            for r in up {
                let _ = writeln!(o, "PostUp = {}", seguro("Regla", &r)?);
            }
            for r in down {
                let _ = writeln!(o, "PostDown = {}", seguro("Regla", &r)?);
            }
        }
        for c in self.clientes_activos() {
            let ip: Ipv4Addr =
                c.ip.trim().parse().map_err(|_| format!("«{}»: IP no válida", c.nombre.escape_debug()))?;
            let _ = writeln!(
                o,
                "\n# {} — {}",
                seguro("Nombre del cliente", &c.nombre)?,
                seguro("Acceso", &self.resumen_acceso(c))?
            );
            let _ = writeln!(o, "[Peer]");
            let _ = writeln!(o, "PublicKey = {}", clave("Clave pública", &c.clave_publica)?);
            if c.usar_psk {
                let _ = writeln!(o, "PresharedKey = {}", clave("Clave precompartida", &c.psk)?);
            }
            let _ = writeln!(o, "AllowedIPs = {ip}/32");
        }
        Ok(o)
    }

    pub fn config_cliente(&self, c: &Cliente) -> Result<String, String> {
        let s = &self.servidor;
        let ip: Ipv4Addr =
            c.ip.trim().parse().map_err(|_| format!("«{}»: IP no válida", c.nombre.escape_debug()))?;
        let mut o = String::new();
        let _ = writeln!(
            o,
            "# {} — {}",
            seguro("Nombre del cliente", &c.nombre)?,
            seguro("Acceso", &self.resumen_acceso(c))?
        );
        let _ = writeln!(o, "[Interface]");
        let _ = writeln!(o, "PrivateKey = {}", clave("Clave privada", &c.clave_privada)?);
        let _ = writeln!(o, "Address = {ip}/32");
        let dns = self.dns()?;
        if c.usar_dns && !dns.is_empty() {
            let d: Vec<String> = dns.iter().map(|d| d.to_string()).collect();
            let _ = writeln!(o, "DNS = {}", d.join(", "));
        }
        if let Some(m) = validar_mtu(&s.mtu)? {
            let _ = writeln!(o, "MTU = {m}");
        }
        let _ = writeln!(o, "\n[Peer]");
        let _ = writeln!(o, "PublicKey = {}", clave("Clave pública del servidor", &s.clave_publica)?);
        if c.usar_psk {
            let _ = writeln!(o, "PresharedKey = {}", clave("Clave precompartida", &c.psk)?);
        }
        let (host, puerto) = partes_endpoint(&s.endpoint)?;
        let puerto = match puerto {
            Some(p) => p,
            None => validar_puerto(&s.puerto)?.unwrap_or_default(),
        };
        // IPv6 literal necesita corchetes
        let host = if host.parse::<std::net::Ipv6Addr>().is_ok() { format!("[{host}]") } else { host };
        let _ = writeln!(o, "Endpoint = {host}:{puerto}");
        let _ = writeln!(o, "AllowedIPs = {}", self.allowed_ips_cliente(c)?);
        if let Ok(Some(n)) = validar_keepalive(&c.keepalive)
            && n > 0
        {
            let _ = writeln!(o, "PersistentKeepalive = {n}");
        }
        Ok(o)
    }

    pub fn resumen_acceso(&self, c: &Cliente) -> String {
        let mut partes = Vec::new();
        if c.internet {
            partes.push("internet".to_string());
        }
        if c.toda_intranet {
            partes.push("toda la intranet".to_string());
        } else {
            for s in self.intranet.iter().filter(|s| c.subredes.contains(&s.id)) {
                partes.push(s.nombre.clone());
            }
        }
        if !c.extra.trim().is_empty() {
            partes.push(format!("extra: {}", c.extra.trim()));
        }
        if partes.is_empty() { "solo red VPN".into() } else { partes.join(", ") }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn proyecto() -> Proyecto {
        let mut p = Proyecto::nuevo();
        p.anadir_subred("Servidores", "192.168.10.0/24");
        let i = p.anadir_cliente();
        p.clientes[i].toda_intranet = false;
        p.clientes[i].subredes = vec![p.intranet[1].id];
        let j = p.anadir_cliente();
        p.clientes[j].internet = true;
        p.clientes[j].toda_intranet = false;
        p
    }

    #[test]
    fn ips_automaticas() {
        let p = proyecto();
        assert_eq!(p.clientes[0].ip, "10.8.0.2");
        assert_eq!(p.clientes[1].ip, "10.8.0.3");
        assert!(p.validar().is_empty(), "{:?}", p.validar());
    }

    #[test]
    fn allowed_ips() {
        let p = proyecto();
        assert_eq!(p.allowed_ips_cliente(&p.clientes[0]).unwrap(), "10.8.0.0/24, 192.168.10.0/24");
        assert_eq!(p.allowed_ips_cliente(&p.clientes[1]).unwrap(), "0.0.0.0/0, ::/0");
    }

    #[test]
    fn firewall_internet_sin_intranet() {
        let p = proyecto();
        let conf = p.config_servidor().unwrap();
        assert!(conf.contains("-s 10.8.0.2/32 -d 192.168.10.0/24 -m comment --comment cliente1 -j ACCEPT"));
        assert!(!conf.contains("-s 10.8.0.2/32 -d 192.168.1.0/24"));
        assert!(conf.contains("-s 10.8.0.3/32 -d 192.168.0.0/16 -m comment --comment cliente2 -j DROP"));
        assert!(conf.contains("-s 10.8.0.3/32 -o eth0 -m comment --comment cliente2 -j ACCEPT"));
        assert!(conf.contains("PostUp = iptables -A wg0-acl -j DROP"));
        assert!(conf.contains("MASQUERADE"));
    }

    #[test]
    fn puerto() {
        assert_eq!(validar_puerto(" 51820 "), Ok(Some(51820)));
        assert!(validar_puerto("").is_err());
        assert!(validar_puerto("51a20").unwrap_err().contains("solo puede contener números"));
        assert!(validar_puerto("-5").unwrap_err().contains("solo puede contener números"));
        assert!(validar_puerto("0").unwrap_err().contains("entre 1 y 65535"));
        assert!(validar_puerto("70000").unwrap_err().contains("entre 1 y 65535"));
        // proyectos antiguos guardaban el puerto como número
        let s: Servidor = serde_json::from_str(r#"{"puerto": 51821}"#).unwrap();
        assert_eq!(s.puerto, "51821");
        let c: Cliente = serde_json::from_str(r#"{"keepalive": 0}"#).unwrap();
        assert_eq!(c.keepalive, "0");
    }

    #[test]
    fn mtu_y_keepalive() {
        assert_eq!(validar_mtu(""), Ok(None));
        assert_eq!(validar_mtu("1420"), Ok(Some(1420)));
        assert!(validar_mtu("14x0").unwrap_err().contains("solo puede contener números"));
        assert!(validar_mtu("100").unwrap_err().contains("entre 576 y 9000"));
        assert_eq!(validar_keepalive(""), Ok(None));
        assert_eq!(validar_keepalive("0"), Ok(Some(0)));
        assert!(validar_keepalive("25s").is_err());
        let mut p = Proyecto::nuevo();
        let i = p.anadir_cliente();
        assert!(p.config_cliente(&p.clientes[i]).unwrap().contains("PersistentKeepalive = 25"));
        p.clientes[i].keepalive = "0".into();
        assert!(!p.config_cliente(&p.clientes[i]).unwrap().contains("PersistentKeepalive"));
        p.clientes[i].keepalive = "x".into();
        assert!(p.validar().iter().any(|e| e.contains("Keepalive")));
    }

    const INYECCION: &str = "x\n[Interface]\nPostUp = id > /tmp/p\n#";

    #[test]
    fn nombre_con_salto_rechazado() {
        let mut p = proyecto();
        p.clientes[0].nombre = INYECCION.into();
        assert!(p.validar().iter().any(|e| e.contains("caracteres de control")), "{:?}", p.validar());
        for (i, v) in ["a\rb", "a\0b", "a\tb", "a\u{7f}b"].iter().enumerate() {
            let mut p = proyecto();
            p.clientes[0].nombre = (*v).into();
            assert!(!p.validar().is_empty(), "caso {i}");
        }
        let mut p = proyecto();
        p.intranet[0].nombre = INYECCION.into();
        assert!(!p.validar().is_empty());
        let mut p = proyecto();
        p.clientes[0].extra = "10.1.0.0/24\nPostUp = id".into();
        assert!(!p.validar().is_empty());
        let mut p = proyecto();
        p.servidor.dns = "1.1.1.1\n".into();
        assert!(!p.validar().is_empty());
    }

    #[test]
    fn endpoint_con_salto_rechazado() {
        let mut p = proyecto();
        p.servidor.endpoint = "vpn.ejemplo.com\nPostUp = id".into();
        assert!(p.validar().iter().any(|e| e.contains("ndpoint")), "{:?}", p.validar());
        for malo in ["vpn ejemplo", "vpn.ejemplo.com:", "vpn.ejemplo.com:0", "vpn.ejemplo.com:99999", "a;b", "[::1", "-x"] {
            let mut p = proyecto();
            p.servidor.endpoint = malo.into();
            assert!(!p.validar().is_empty(), "{malo} debería rechazarse");
        }
        for bueno in ["vpn.ejemplo.com", "203.0.113.4", "203.0.113.4:4500", "2001:db8::1", "[2001:db8::1]:4500", "mi-vpn.example.org:51820"] {
            let mut p = proyecto();
            p.servidor.endpoint = bueno.into();
            assert!(p.validar().is_empty(), "{bueno}: {:?}", p.validar());
        }
        // Con puerto propio, el endpoint usa ese puerto (NAT con otro puerto externo).
        let mut p = proyecto();
        p.servidor.endpoint = "vpn.ejemplo.com:4500".into();
        assert!(p.config_cliente(&p.clientes[0]).unwrap().contains("Endpoint = vpn.ejemplo.com:4500\n"));
        p.servidor.endpoint = "2001:db8::1".into();
        assert!(p.config_cliente(&p.clientes[0]).unwrap().contains("Endpoint = [2001:db8::1]:51820\n"));
    }

    #[test]
    fn interfaz_invalida_rechazada() {
        for mala in ["../../root/x", "..", ".", "wg0\n", "wg 0", "una-interfaz-larga", "wg/0", ""] {
            let mut p = proyecto();
            p.servidor.interfaz = mala.into();
            assert!(!p.validar().is_empty(), "interfaz {mala:?}");
            let mut p = proyecto();
            p.servidor.iface_salida = mala.into();
            assert!(!p.validar().is_empty(), "iface_salida {mala:?}");
        }
        assert!(validar_interfaz("Interfaz", "wg-0.lan_1").is_ok());
        // Claves: base64 de 32 bytes.
        let mut p = proyecto();
        p.clientes[0].clave_publica = "no es base64\nPostUp = id".into();
        assert!(!p.validar().is_empty());
        let mut p = proyecto();
        p.servidor.clave_publica = "QUJD".into();
        assert!(!p.validar().is_empty());
    }

    #[test]
    fn conf_nunca_contiene_postup_inyectado() {
        // Aunque alguien se salte validar(), los generadores no escriben saltos de línea.
        let mut p = proyecto();
        p.clientes[0].nombre = INYECCION.into();
        assert!(p.config_servidor_panel().is_err());
        assert!(p.config_servidor().is_err());
        assert!(p.config_cliente(&p.clientes[0]).is_err());
        let mut p = proyecto();
        p.servidor.endpoint = "vpn\nPostUp = id > /tmp/p".into();
        assert!(p.config_cliente(&p.clientes[0]).is_err());
        let mut p = proyecto();
        p.clientes[1].clave_publica = "x\nPostUp = id".into();
        assert!(p.config_servidor_panel().is_err());
        let mut p = proyecto();
        p.intranet[0].nombre = INYECCION.into();
        p.clientes[0].subredes = vec![p.intranet[0].id];
        assert!(p.config_cliente(&p.clientes[0]).is_err());
        // Reglas iptables (app de escritorio): interfaces y IPs validadas.
        let mut p = proyecto();
        p.servidor.iface_salida = "eth0; id > /tmp/p".into();
        assert!(p.config_servidor().is_err());
        let mut p = proyecto();
        p.servidor.interfaz = "wg0;id".into();
        assert!(p.config_servidor().is_err());
        let mut p = proyecto();
        p.clientes[0].ip = "10.8.0.2;id".into();
        assert!(p.config_servidor().is_err());
        // Un proyecto válido no tiene PostUp ajenos.
        let p = proyecto();
        let c = p.config_servidor().unwrap();
        assert!(c.lines().filter(|l| l.starts_with("PostUp")).all(|l| l.starts_with("PostUp = iptables") || l.starts_with("PostUp = sysctl")), "{c}");
    }

    #[test]
    fn detecta_errores() {
        let mut p = proyecto();
        p.clientes[1].ip = "10.8.0.2".into();
        p.clientes[0].extra = "nope".into();
        let e = p.validar();
        assert!(e.iter().any(|x| x.contains("repetida")));
        assert!(e.iter().any(|x| x.contains("nope")));
    }
}
