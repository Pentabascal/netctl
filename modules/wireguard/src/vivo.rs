//! Estado en vivo de los clientes: `wg show <iface> dump` cada pocos segundos.

use std::collections::VecDeque;

use serde::Serialize;

/// Un cliente cuenta como conectado si su último saludo fue hace menos de esto.
pub const CONECTADO_SEGUNDOS: u64 = 180;
/// Cada cuánto se consulta `wg show`.
pub const INTERVALO_SEGUNDOS: u64 = 5;
/// Muestras de tráfico que se guardan para la gráfica (5 min a 5 s).
pub const HISTORIAL: usize = 60;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
pub struct EstadoPeer {
    pub clave_publica: String,
    /// Dirección real desde la que conecta (`None` si nunca ha conectado).
    pub endpoint: Option<String>,
    /// Instante UNIX del último saludo; `None` si nunca ha conectado.
    pub ultimo_handshake: Option<u64>,
    /// Bytes recibidos del cliente / enviados al cliente.
    pub rx: u64,
    pub tx: u64,
    pub conectado: bool,
    /// Bytes por segundo desde la muestra anterior.
    pub tasa_rx: u64,
    pub tasa_tx: u64,
}

/// Interpreta la salida de `wg show <iface> dump`. La primera línea es la interfaz (4 campos);
/// cada peer ocupa una línea de 8 campos separados por tabuladores.
pub fn parsear_dump(salida: &str, ahora: u64) -> Vec<EstadoPeer> {
    salida
        .lines()
        .filter_map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            if c.len() != 8 {
                return None;
            }
            let num = |s: &str| s.trim().parse::<u64>().ok();
            let hs = num(c[4])?;
            let ultimo = (hs > 0).then_some(hs);
            Some(EstadoPeer {
                clave_publica: c[0].to_string(),
                endpoint: (c[2] != "(none)" && !c[2].is_empty()).then(|| c[2].to_string()),
                ultimo_handshake: ultimo,
                rx: num(c[5]).unwrap_or(0),
                tx: num(c[6]).unwrap_or(0),
                conectado: ultimo.is_some_and(|t| ahora.saturating_sub(t) < CONECTADO_SEGUNDOS),
                ..Default::default()
            })
        })
        .collect()
}

#[derive(Serialize, Clone, Copy, Debug, Default, PartialEq)]
pub struct Muestra {
    pub t: u64,
    /// Bytes por segundo, sumando todos los clientes.
    pub rx: u64,
    pub tx: u64,
}

/// Última foto del estado de la interfaz.
#[derive(Serialize, Clone, Debug, Default)]
pub struct Vivo {
    pub interfaz: String,
    pub activa: bool,
    /// Texto para mostrar si la interfaz no está activa.
    pub aviso: Option<String>,
    pub momento: u64,
    pub peers: Vec<EstadoPeer>,
    pub historial: VecDeque<Muestra>,
}

impl Vivo {
    pub fn peer(&self, clave: &str) -> Option<&EstadoPeer> {
        self.peers.iter().find(|p| p.clave_publica == clave)
    }

    /// Nueva foto a partir de la anterior (para calcular tasas) y del resultado de `wg show`.
    pub fn siguiente(&self, interfaz: &str, resultado: Result<String, String>, ahora: u64) -> Vivo {
        let mismo = self.interfaz == interfaz;
        let mut historial = if mismo {
            self.historial.clone()
        } else {
            VecDeque::new()
        };
        let Ok(salida) = resultado else {
            return Vivo {
                interfaz: interfaz.into(),
                activa: false,
                aviso: Some(format!("La interfaz {interfaz} no está activa")),
                momento: ahora,
                peers: vec![],
                historial,
            };
        };
        let dt = ahora.saturating_sub(self.momento);
        let mut peers = parsear_dump(&salida, ahora);
        let (mut total_rx, mut total_tx) = (0, 0);
        if mismo && self.activa && dt > 0 {
            for p in &mut peers {
                if let Some(a) = self.peer(&p.clave_publica) {
                    p.tasa_rx = p.rx.saturating_sub(a.rx) / dt;
                    p.tasa_tx = p.tx.saturating_sub(a.tx) / dt;
                }
                total_rx += p.tasa_rx;
                total_tx += p.tasa_tx;
            }
        }
        historial.push_back(Muestra {
            t: ahora,
            rx: total_rx,
            tx: total_tx,
        });
        while historial.len() > HISTORIAL {
            historial.pop_front();
        }
        Vivo {
            interfaz: interfaz.into(),
            activa: true,
            aviso: None,
            momento: ahora,
            peers,
            historial,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AHORA: u64 = 1_700_000_000;

    fn dump(handshake: u64) -> String {
        format!(
            "cHJpdg==\tcHVi\t51820\toff\n\
             QUJD\t(none)\t198.51.100.4:51000\t10.8.0.2/32\t{handshake}\t1024\t2048\t25\n"
        )
    }

    #[test]
    fn dump_conectado() {
        let v = parsear_dump(&dump(AHORA - 30), AHORA);
        assert_eq!(v.len(), 1);
        let p = &v[0];
        assert_eq!(p.clave_publica, "QUJD");
        assert_eq!(p.endpoint.as_deref(), Some("198.51.100.4:51000"));
        assert_eq!(p.ultimo_handshake, Some(AHORA - 30));
        assert_eq!((p.rx, p.tx), (1024, 2048));
        assert!(p.conectado);
    }

    #[test]
    fn dump_viejo() {
        let v = parsear_dump(&dump(AHORA - 200), AHORA);
        assert!(!v[0].conectado);
        assert_eq!(v[0].ultimo_handshake, Some(AHORA - 200));
    }

    #[test]
    fn dump_nunca() {
        let s = "cHJpdg==\tcHVi\t51820\toff\nQUJD\t(none)\t(none)\t10.8.0.2/32\t0\t0\t0\toff\n";
        let v = parsear_dump(s, AHORA);
        assert_eq!(v[0].ultimo_handshake, None);
        assert_eq!(v[0].endpoint, None);
        assert!(!v[0].conectado);
    }

    #[test]
    fn dump_vacio_o_raro() {
        assert!(parsear_dump("", AHORA).is_empty());
        assert!(parsear_dump("basura\n", AHORA).is_empty());
    }

    #[test]
    fn tasas_e_historial() {
        let a = Vivo::default().siguiente("wg0", Ok(dump(AHORA - 30)), AHORA);
        let s = "x\ty\t1\toff\nQUJD\t(none)\t1.2.3.4:5\t10.8.0.2/32\t1\t6144\t2048\t25\n";
        let b = a.siguiente("wg0", Ok(s.into()), AHORA + 5);
        assert_eq!(b.peers[0].tasa_rx, 1024);
        assert_eq!(b.historial.len(), 2);
        assert_eq!(b.historial[1].rx, 1024);
        let c = b.siguiente("wg0", Err("No such device".into()), AHORA + 10);
        assert!(!c.activa);
        assert_eq!(c.aviso.as_deref(), Some("La interfaz wg0 no está activa"));
    }
}
