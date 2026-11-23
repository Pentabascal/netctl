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
        if s.interfaz.trim().is_empty() {
            e.push("Falta el nombre de interfaz del servidor".into());
        }
        if s.endpoint.trim().is_empty() {
            e.push("Falta el endpoint público del servidor (IP o dominio)".into());
        }
        if let Err(x) = validar_puerto(&s.puerto) {
            e.push(x);
        }
        if (s.nat || s.firewall) && s.iface_salida.trim().is_empty() {
            e.push("Falta la interfaz de salida (LAN/internet) del servidor".into());
        }
        if claves::publica_de(&s.clave_privada).is_none() {
            e.push("La clave privada del servidor no es válida".into());
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
        let wg = s.interfaz.trim();
        let out = s.iface_salida.trim();
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
                let ip = c.ip.trim();
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
        let mut o = String::new();
        if con_reglas {
            let _ = writeln!(o, "# Servidor WireGuard — generado por wg-configurador");
            let _ = writeln!(o, "# Guardar como /etc/wireguard/{}.conf (chmod 600)", s.interfaz.trim());
            let _ = writeln!(o, "# Activar: systemctl enable --now wg-quick@{}\n", s.interfaz.trim());
        } else {
            let _ = writeln!(o, "# Servidor WireGuard — gestionado por netctl, no editar a mano\n");
        }
        let _ = writeln!(o, "[Interface]");
        let _ = writeln!(o, "Address = {}/{}", s.ip_servidor.trim(), red.prefix_len());
        let _ = writeln!(o, "ListenPort = {}", s.puerto.trim());
        let _ = writeln!(o, "PrivateKey = {}", s.clave_privada.trim());
        if !s.mtu.trim().is_empty() {
            let _ = writeln!(o, "MTU = {}", s.mtu.trim());
        }
        if con_reglas {
            let (up, down) = self.reglas()?;
            for r in up {
                let _ = writeln!(o, "PostUp = {r}");
            }
            for r in down {
                let _ = writeln!(o, "PostDown = {r}");
            }
        }
        for c in self.clientes_activos() {
            let _ = writeln!(o, "\n# {} — {}", c.nombre, self.resumen_acceso(c));
            let _ = writeln!(o, "[Peer]");
            let _ = writeln!(o, "PublicKey = {}", c.clave_publica.trim());
            if c.usar_psk {
                let _ = writeln!(o, "PresharedKey = {}", c.psk.trim());
            }
            let _ = writeln!(o, "AllowedIPs = {}/32", c.ip.trim());
        }
        Ok(o)
    }

    pub fn config_cliente(&self, c: &Cliente) -> Result<String, String> {
        let s = &self.servidor;
        let mut o = String::new();
        let _ = writeln!(o, "# {} — {}", c.nombre, self.resumen_acceso(c));
        let _ = writeln!(o, "[Interface]");
        let _ = writeln!(o, "PrivateKey = {}", c.clave_privada.trim());
        let _ = writeln!(o, "Address = {}/32", c.ip.trim());
        let dns = self.dns()?;
        if c.usar_dns && !dns.is_empty() {
            let d: Vec<String> = dns.iter().map(|d| d.to_string()).collect();
            let _ = writeln!(o, "DNS = {}", d.join(", "));
        }
        if !s.mtu.trim().is_empty() {
            let _ = writeln!(o, "MTU = {}", s.mtu.trim());
        }
        let _ = writeln!(o, "\n[Peer]");
        let _ = writeln!(o, "PublicKey = {}", s.clave_publica.trim());
        if c.usar_psk {
            let _ = writeln!(o, "PresharedKey = {}", c.psk.trim());
        }
        let host = s.endpoint.trim();
        // IPv6 literal necesita corchetes
        let host = if host.parse::<std::net::Ipv6Addr>().is_ok() { format!("[{host}]") } else { host.to_string() };
        let _ = writeln!(o, "Endpoint = {host}:{}", s.puerto.trim());
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
