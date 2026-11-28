//! Modelo del firewall: zonas, interfaces, políticas zona→zona, reglas, grupos y reenvíos.

use std::collections::BTreeMap;
use std::net::{IpAddr, Ipv4Addr};

use ipnet::IpNet;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TipoZona {
    Externa,
    Interna,
    Gateway,
    Vpn,
    Dmz,
    Personalizada,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Zona {
    pub id: String,
    pub nombre: String,
    pub tipo: TipoZona,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Politica {
    Permitir,
    Bloquear,
}

/// Política de una casilla de la matriz que el usuario cambió (las demás usan la de serie).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PoliticaPar {
    pub origen: String,
    pub destino: String,
    pub politica: Politica,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Accion {
    Permitir,
    Bloquear,
    Rechazar,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Protocolo {
    #[default]
    Cualquiera,
    Tcp,
    Udp,
    TcpUdp,
    Icmp,
}

impl Protocolo {
    pub fn usa_puertos(self) -> bool {
        matches!(self, Protocolo::Tcp | Protocolo::Udp | Protocolo::TcpUdp)
    }
    pub fn incluye_tcp(self) -> bool {
        matches!(self, Protocolo::Tcp | Protocolo::TcpUdp)
    }
    pub fn incluye_udp(self) -> bool {
        matches!(self, Protocolo::Udp | Protocolo::TcpUdp)
    }
}

/// Un puerto (`desde == hasta`) o un rango.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangoPuerto {
    pub desde: u16,
    pub hasta: u16,
}

impl RangoPuerto {
    pub fn uno(p: u16) -> Self {
        Self { desde: p, hasta: p }
    }
    pub fn solapa(&self, o: &RangoPuerto) -> bool {
        self.desde <= o.hasta && o.desde <= self.hasta
    }
    pub fn contiene(&self, p: u16) -> bool {
        self.desde <= p && p <= self.hasta
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "tipo", content = "valor", rename_all = "snake_case")]
pub enum Puertos {
    #[default]
    Cualquiera,
    Lista(Vec<RangoPuerto>),
    Grupo(u64),
}

/// Origen o destino: lo que devuelve el selector de equipos (§7.6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
#[serde(tag = "tipo", rename_all = "snake_case")]
pub enum Direccion {
    #[default]
    Cualquiera,
    /// Vinculado por MAC: si el equipo cambia de IP, se actualiza `ip`.
    Equipo { mac: String, ip: IpAddr, nombre: String },
    Ip { red: IpNet },
    Rango { desde: IpAddr, hasta: IpAddr },
    Grupo { id: u64 },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Regla {
    pub id: u64,
    pub nombre: String,
    pub origen_zona: String,
    pub destino_zona: String,
    pub accion: Accion,
    #[serde(default)]
    pub protocolo: Protocolo,
    #[serde(default)]
    pub puertos: Puertos,
    #[serde(default)]
    pub origen: Direccion,
    #[serde(default)]
    pub destino: Direccion,
    #[serde(default)]
    pub log: bool,
    #[serde(default = "si")]
    pub activa: bool,
    #[serde(default)]
    pub notas: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrupoIp {
    pub id: u64,
    pub nombre: String,
    pub miembros: Vec<Direccion>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GrupoPuertos {
    pub id: u64,
    pub nombre: String,
    pub puertos: Vec<RangoPuerto>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reenvio {
    pub id: u64,
    pub nombre: String,
    /// Equipo de destino (`Equipo` o `Ip` de una sola dirección IPv4).
    pub equipo: Direccion,
    pub protocolo: Protocolo,
    /// Puertos de fuera (uno, varios o un rango).
    pub externo: Vec<RangoPuerto>,
    /// Puerto de dentro si es distinto (solo con un único puerto de fuera).
    #[serde(default)]
    pub interno: Option<u16>,
    /// Interfaz WAN; `None` = todas las de la zona Externa.
    #[serde(default)]
    pub wan: Option<String>,
    /// Vacío = desde cualquier sitio de internet.
    #[serde(default)]
    pub desde: Vec<Direccion>,
    /// Segundos UNIX; `None` = nunca caduca.
    #[serde(default)]
    pub caduca: Option<u64>,
    #[serde(default = "si")]
    pub loopback: bool,
    #[serde(default = "si")]
    pub activo: bool,
    #[serde(default)]
    pub log: bool,
    #[serde(default)]
    pub plantilla: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub revision: u64,
    pub zonas: Vec<Zona>,
    /// interfaz → id de zona
    pub interfaces: BTreeMap<String, String>,
    #[serde(default)]
    pub politicas: Vec<PoliticaPar>,
    #[serde(default)]
    pub reglas: Vec<Regla>,
    #[serde(default)]
    pub grupos_ip: Vec<GrupoIp>,
    #[serde(default)]
    pub grupos_puertos: Vec<GrupoPuertos>,
    #[serde(default)]
    pub reenvios: Vec<Reenvio>,
    /// zona de origen → enmascarar al salir por Externa
    #[serde(default)]
    pub nat: BTreeMap<String, bool>,
    #[serde(default = "si")]
    pub ping_externo: bool,
    #[serde(default)]
    pub sig_id: u64,
    /// Direcciones de cada interfaz (se refrescan al arrancar; sirven para los avisos y el
    /// NAT loopback).
    #[serde(default)]
    pub direcciones: BTreeMap<String, Vec<IpNet>>,
}

fn si() -> bool {
    true
}

/// Lo que el firewall siempre deja pasar para no dejar a nadie fuera.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fijas {
    pub panel: u16,
    pub alcance_internet: bool,
    pub ssh: u16,
    /// (interfaz WireGuard, puerto UDP)
    pub wg: Vec<(String, u16)>,
    /// IPv4 de las interfaces de la zona Externa (para el NAT loopback y «cómo conectarse»).
    #[serde(default)]
    pub ips_externas: Vec<Ipv4Addr>,
}

/// Interfaz de red tal como la ve el sistema.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Iface {
    pub nombre: String,
    pub ips: Vec<IpNet>,
    pub mac: Option<String>,
    pub activa: bool,
}

pub const ZONAS_DE_SERIE: [(&str, &str, TipoZona); 5] = [
    ("externa", "Externa (internet)", TipoZona::Externa),
    ("interna", "Interna", TipoZona::Interna),
    ("gateway", "Esta máquina", TipoZona::Gateway),
    ("vpn", "VPN", TipoZona::Vpn),
    ("dmz", "DMZ", TipoZona::Dmz),
];

impl Config {
    /// Configuración inicial recomendada a partir de las interfaces de la máquina.
    pub fn por_defecto(ifaces: &[Iface], ruta_defecto: Option<&str>) -> Config {
        let reales: Vec<&Iface> = ifaces.iter().filter(|i| i.nombre != "lo").collect();
        let fisicas: Vec<&&Iface> = reales.iter().filter(|i| !es_vpn(&i.nombre)).collect();
        let mut interfaces = BTreeMap::new();
        for i in &reales {
            let zona = if es_vpn(&i.nombre) {
                "vpn"
            } else if fisicas.len() == 1 || Some(i.nombre.as_str()) == ruta_defecto {
                "externa"
            } else {
                "interna"
            };
            interfaces.insert(i.nombre.clone(), zona.to_string());
        }
        let zonas = ZONAS_DE_SERIE
            .iter()
            .map(|(id, nombre, tipo)| Zona { id: (*id).into(), nombre: (*nombre).into(), tipo: *tipo })
            .collect();
        let nat = ["interna", "vpn", "dmz"].iter().map(|z| (z.to_string(), true)).collect();
        Config {
            revision: 0,
            zonas,
            interfaces,
            politicas: vec![],
            reglas: vec![],
            grupos_ip: vec![],
            grupos_puertos: vec![],
            reenvios: vec![],
            nat,
            ping_externo: true,
            sig_id: 0,
            direcciones: reales.iter().map(|i| (i.nombre.clone(), i.ips.clone())).collect(),
        }
    }

    pub fn zona(&self, id: &str) -> Option<&Zona> {
        self.zonas.iter().find(|z| z.id == id)
    }

    /// Interfaces asignadas a la zona `id`.
    pub fn ifaces_de(&self, id: &str) -> Vec<String> {
        self.interfaces.iter().filter(|(_, z)| *z == id).map(|(i, _)| i.clone()).collect()
    }

    /// Ids de zonas del tipo dado.
    pub fn zonas_tipo(&self, t: TipoZona) -> Vec<String> {
        self.zonas.iter().filter(|z| z.tipo == t).map(|z| z.id.clone()).collect()
    }

    /// Interfaces de todas las zonas Externa.
    pub fn ifaces_externas(&self) -> Vec<String> {
        self.zonas_tipo(TipoZona::Externa).iter().flat_map(|z| self.ifaces_de(z)).collect()
    }

    /// Política de la casilla origen→destino (la del usuario o la de serie).
    pub fn politica(&self, origen: &str, destino: &str) -> Politica {
        if let Some(p) = self.politicas.iter().find(|p| p.origen == origen && p.destino == destino) {
            return p.politica;
        }
        match (self.zona(origen), self.zona(destino)) {
            (Some(o), Some(d)) => politica_de_serie(o.tipo, d.tipo, o.id == d.id),
            _ => Politica::Bloquear,
        }
    }

    /// Solo hay una tarjeta de red (sin contar loopback ni VPN): la matriz se reduce a
    /// «qué puede entrar a esta máquina».
    pub fn una_interfaz(&self) -> bool {
        let vpn = self.zonas_tipo(TipoZona::Vpn);
        self.interfaces.iter().filter(|(i, z)| !vpn.contains(z) && !es_vpn(i)).count() <= 1
    }

    pub fn nuevo_id(&mut self) -> u64 {
        self.sig_id += 1;
        self.sig_id
    }
}

/// Política de serie entre dos tipos de zona (tabla del plan de la fase 2).
pub fn politica_de_serie(origen: TipoZona, destino: TipoZona, misma_zona: bool) -> Politica {
    use Politica::*;
    use TipoZona::*;
    match (origen, destino) {
        (Gateway, _) | (Interna, _) => Permitir,
        (Externa, _) => Bloquear,
        (Vpn, Personalizada) => Bloquear,
        (Vpn, _) => Permitir,
        (_, Externa) => Permitir,
        (Dmz, Dmz) => Permitir,
        (Personalizada, Personalizada) if misma_zona => Permitir,
        _ => Bloquear,
    }
}

/// Interfaces WireGuard (se asignan solas a la zona VPN).
pub fn es_vpn(iface: &str) -> bool {
    iface.starts_with("wg")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::*;

    #[test]
    fn zonas_sugeridas() {
        let c = router();
        assert_eq!(c.interfaces.get("eth0").unwrap(), "externa");
        assert_eq!(c.interfaces.get("eth1").unwrap(), "interna");
        assert_eq!(c.interfaces.get("wg0").unwrap(), "vpn");
        assert!(!c.interfaces.contains_key("lo"));
        assert_eq!(c.zonas.len(), 5);
        assert!(!c.una_interfaz());
        assert_eq!(c.nat.get("interna"), Some(&true));
        assert_eq!(c.nat.get("vpn"), Some(&true));
        assert_eq!(c.nat.get("dmz"), Some(&true));
    }

    #[test]
    fn una_interfaz() {
        // Sin ruta por defecto y con IP privada: sigue siendo Externa si es la única.
        let c = Config::por_defecto(&[iface("lo", "127.0.0.1/8"), iface("ens18", "192.168.1.5/24"), iface("wg0", "10.8.0.1/24")], None);
        assert_eq!(c.interfaces.get("ens18").unwrap(), "externa");
        assert!(c.una_interfaz());
    }

    #[test]
    fn matriz_por_defecto() {
        let c = router();
        assert_eq!(c.politica("externa", "gateway"), Politica::Bloquear);
        assert_eq!(c.politica("externa", "interna"), Politica::Bloquear);
        assert_eq!(c.politica("interna", "externa"), Politica::Permitir);
        assert_eq!(c.politica("vpn", "interna"), Politica::Permitir);
        assert_eq!(c.politica("dmz", "interna"), Politica::Bloquear);
        assert_eq!(c.politica("dmz", "externa"), Politica::Permitir);
        assert_eq!(c.politica("gateway", "externa"), Politica::Permitir);
        assert_eq!(politica_de_serie(TipoZona::Personalizada, TipoZona::Personalizada, true), Politica::Permitir);
        assert_eq!(politica_de_serie(TipoZona::Personalizada, TipoZona::Personalizada, false), Politica::Bloquear);
        assert_eq!(politica_de_serie(TipoZona::Vpn, TipoZona::Personalizada, false), Politica::Bloquear);
    }

    #[test]
    fn politica_del_usuario_gana() {
        let mut c = router();
        c.politicas.push(PoliticaPar { origen: "externa".into(), destino: "gateway".into(), politica: Politica::Permitir });
        assert_eq!(c.politica("externa", "gateway"), Politica::Permitir);
    }

    #[test]
    fn json_ida_y_vuelta() {
        let mut c = router();
        c.reenvios.push(reenvio(1, "192.168.1.20", 25565));
        let j = serde_json::to_string(&c).unwrap();
        assert_eq!(serde_json::from_str::<Config>(&j).unwrap(), c);
    }
}
