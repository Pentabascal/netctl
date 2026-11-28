//! Servicios habituales para el asistente de reenvío de puertos.

use serde::Serialize;

use crate::modelo::{Protocolo, RangoPuerto};

#[derive(Clone, Debug, Serialize)]
pub struct Plantilla {
    pub id: &'static str,
    pub nombre: &'static str,
    pub protocolo: Protocolo,
    pub puertos: Vec<RangoPuerto>,
    /// Peligroso abierto a todo internet.
    pub delicado: bool,
    pub icono: &'static str,
}

fn p(id: &'static str, nombre: &'static str, protocolo: Protocolo, puertos: &[u16], delicado: bool, icono: &'static str) -> Plantilla {
    Plantilla { id, nombre, protocolo, puertos: puertos.iter().map(|x| RangoPuerto::uno(*x)).collect(), delicado, icono }
}

pub fn todas() -> Vec<Plantilla> {
    use Protocolo::*;
    vec![
        p("web", "Web (HTTP y HTTPS)", Tcp, &[80, 443], false, "web"),
        p("minecraft", "Minecraft", Tcp, &[25565], false, "juego"),
        p("juegos", "Servidor de juegos (Steam/Source)", TcpUdp, &[27015], false, "juego"),
        p("plex", "Plex", Tcp, &[32400], false, "multimedia"),
        p("jellyfin", "Jellyfin", Tcp, &[8096], false, "multimedia"),
        p("camaras", "Cámaras (RTSP)", Tcp, &[554], false, "camara"),
        p("ssh", "SSH", Tcp, &[22], true, "terminal"),
        p("escritorio_remoto", "Escritorio remoto (RDP)", TcpUdp, &[3389], true, "pantalla"),
        p("ftp", "FTP", Tcp, &[21], false, "carpeta"),
        p("correo", "Correo", Tcp, &[25, 465, 587, 993], false, "correo"),
        p("bases_datos", "Bases de datos (MySQL y PostgreSQL)", Tcp, &[3306, 5432], true, "base_datos"),
        p("otro", "Otro", TcpUdp, &[], false, "mas"),
    ]
}

/// (puerto, nombre del servicio) que no conviene abrir a todo internet.
const DELICADOS: [(u16, &str); 9] = [
    (22, "SSH"),
    (23, "Telnet"),
    (445, "Carpetas compartidas de Windows (SMB)"),
    (3389, "Escritorio remoto"),
    (5900, "Escritorio remoto VNC"),
    (3306, "Base de datos MySQL"),
    (5432, "Base de datos PostgreSQL"),
    (1433, "Base de datos SQL Server"),
    (6379, "Base de datos Redis"),
];

pub fn buscar(id: &str) -> Option<Plantilla> {
    todas().into_iter().find(|p| p.id == id)
}

/// ¿Algún puerto de la lista es de un servicio delicado (SSH, escritorio remoto, BD…)?
pub fn puerto_delicado(puertos: &[RangoPuerto]) -> Option<&'static str> {
    DELICADOS.iter().find(|(d, _)| puertos.iter().any(|r| r.contiene(*d))).map(|(_, n)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plantillas_basicas() {
        let w = buscar("web").unwrap();
        assert_eq!(w.puertos, vec![RangoPuerto::uno(80), RangoPuerto::uno(443)]);
        assert_eq!(w.protocolo, Protocolo::Tcp);
        assert!(!w.delicado);
        assert!(buscar("escritorio_remoto").unwrap().delicado);
        assert!(buscar("ssh").unwrap().delicado);
        assert_eq!(buscar("juegos").unwrap().protocolo, Protocolo::TcpUdp);
        assert!(buscar("otro").is_some());
        let ids: Vec<_> = todas().iter().map(|p| p.id).collect();
        for id in ["minecraft", "plex", "jellyfin", "camaras", "ftp", "correo", "bases_datos"] {
            assert!(ids.contains(&id), "falta {id}");
        }
    }

    #[test]
    fn detecta_delicados() {
        assert_eq!(puerto_delicado(&[RangoPuerto::uno(3389)]), Some("Escritorio remoto"));
        assert!(puerto_delicado(&[RangoPuerto { desde: 3000, hasta: 4000 }]).is_some());
        assert_eq!(puerto_delicado(&[RangoPuerto::uno(25565)]), None);
    }
}
