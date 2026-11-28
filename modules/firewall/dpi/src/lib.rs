//! `netctl-dpi`: ayudante del Firewall de netctl. Recibe por NFQUEUE los primeros paquetes de
//! cada conexión de las parejas de zonas con reglas de App, los pasa a la libndpi del sistema
//! (cargada en tiempo de ejecución) y, cuando sabe la app, devuelve el paquete con la marca
//! `0x10000 | app`; nftables la copia a `ct mark` y las reglas comparan ese `ct mark`.
//!
//! Es un binario aparte, con glibc, porque el módulo es musl estático y no puede cargar
//! librerías. Si se cae, el tráfico sigue pasando (la cola lleva `bypass`).

pub mod cola;
pub mod ndpi;

use std::collections::HashMap;

/// Marca que lleva un paquete ya clasificado: bit 16 + id de app de nDPI.
pub const MARCA: u32 = 0x10000;
/// Paquetes que se miran como mucho por conexión (igual que `ct original packets < 13`).
pub const MAX_PAQUETES: u32 = 12;
/// Una conexión sin paquetes en este tiempo se olvida (ms).
pub const OLVIDO_MS: u64 = 120_000;

/// Algo que sabe decir la app de una conexión mirando sus paquetes (nDPI o uno falso).
pub trait Clasificador: Send {
    /// Pasa un paquete (cabecera IP incluida) de la conexión `flujo`. Devuelve la app cuando
    /// está decidida.
    fn paquete(&mut self, flujo: u64, datos: &[u8], ahora_ms: u64) -> Option<u16>;
    /// Lo mejor que se sabe al rendirse (0 = desconocida).
    fn rendirse(&mut self, _flujo: u64) -> u16 {
        0
    }
    /// Libera lo que se guardaba de la conexión.
    fn olvidar(&mut self, _flujo: u64) {}
}

struct Flujo {
    paquetes: u32,
    ultimo_ms: u64,
}

/// Lleva la cuenta de las conexiones y decide la marca de cada paquete.
pub struct Motor<C: Clasificador> {
    pub clasificador: C,
    flujos: HashMap<u64, Flujo>,
    vistos: u64,
}

impl<C: Clasificador> Motor<C> {
    pub fn new(clasificador: C) -> Self {
        Self { clasificador, flujos: HashMap::new(), vistos: 0 }
    }

    /// Marca para el paquete: 0 = aún no se sabe (sigue sin marca y llegará el siguiente).
    pub fn marca(&mut self, flujo: u64, datos: &[u8], ahora_ms: u64) -> u32 {
        self.vistos += 1;
        if self.vistos.is_multiple_of(1024) {
            self.limpiar(ahora_ms);
        }
        let f = self.flujos.entry(flujo).or_insert(Flujo { paquetes: 0, ultimo_ms: ahora_ms });
        f.paquetes += 1;
        f.ultimo_ms = ahora_ms;
        let n = f.paquetes;
        let app = match self.clasificador.paquete(flujo, datos, ahora_ms) {
            Some(a) => Some(a),
            None if n >= MAX_PAQUETES => Some(self.clasificador.rendirse(flujo)),
            None => None,
        };
        match app {
            Some(a) => {
                self.flujos.remove(&flujo);
                self.clasificador.olvidar(flujo);
                MARCA | u32::from(a)
            }
            None => 0,
        }
    }

    pub fn limpiar(&mut self, ahora_ms: u64) {
        let viejos: Vec<u64> = self.flujos.iter().filter(|(_, f)| f.ultimo_ms + OLVIDO_MS < ahora_ms).map(|(k, _)| *k).collect();
        for k in viejos {
            self.flujos.remove(&k);
            self.clasificador.olvidar(k);
        }
    }

    pub fn abiertos(&self) -> usize {
        self.flujos.len()
    }
}

/// Clave de conexión a partir de la cabecera IP (los dos sentidos dan la misma clave). Para
/// cuando el kernel no da el id de conntrack.
pub fn clave_flujo(p: &[u8]) -> Option<u64> {
    use std::hash::{Hash, Hasher};
    let v = p.first()? >> 4;
    let (proto, src, dst, l4): (u8, &[u8], &[u8], usize) = match v {
        4 => {
            let ihl = usize::from(p.first()? & 0x0f) * 4;
            (*p.get(9)?, p.get(12..16)?, p.get(16..20)?, ihl)
        }
        6 => (*p.get(6)?, p.get(8..24)?, p.get(24..40)?, 40),
        _ => return None,
    };
    let puertos = if proto == 6 || proto == 17 { p.get(l4..l4 + 4)? } else { &[0, 0, 0, 0][..] };
    let a = (src, &puertos[0..2]);
    let b = (dst, &puertos[2..4]);
    let (x, y) = if a <= b { (a, b) } else { (b, a) };
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (proto, x, y).hash(&mut h);
    Some(h.finish())
}

/// Versiones de nDPI con las que se ha comprobado la ABI: 4.14 y posteriores, hasta la 6.
pub fn comprobar_version(rev: &str) -> Result<(), String> {
    let mut p = rev.trim().split(|c: char| !c.is_ascii_digit()).filter(|x| !x.is_empty());
    let mayor: u32 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    let menor: u32 = p.next().and_then(|x| x.parse().ok()).unwrap_or(0);
    match (mayor, menor) {
        (4, m) if m >= 14 => Ok(()),
        (5 | 6, _) => Ok(()),
        _ => Err(format!(
            "La versión {} de nDPI no está soportada: hace falta la 4.14 o una 5.x o 6.x.",
            rev.trim().chars().take(24).collect::<String>()
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decide la app 7 al tercer paquete de cada conexión; anota lo que olvida.
    #[derive(Default)]
    struct ClasificadorFalso {
        vistos: HashMap<u64, u32>,
        olvidados: Vec<u64>,
    }
    impl Clasificador for ClasificadorFalso {
        fn paquete(&mut self, flujo: u64, datos: &[u8], _: u64) -> Option<u16> {
            if datos == b"nunca" {
                return None;
            }
            let n = self.vistos.entry(flujo).or_default();
            *n += 1;
            (*n == 3).then_some(7)
        }
        fn rendirse(&mut self, _: u64) -> u16 {
            5
        }
        fn olvidar(&mut self, f: u64) {
            self.olvidados.push(f);
        }
    }

    #[test]
    fn clasificador_falso_marca() {
        let mut m = Motor::new(ClasificadorFalso::default());
        assert_eq!(m.marca(1, b"a", 0), 0);
        assert_eq!(m.marca(1, b"a", 1), 0);
        assert_eq!(m.marca(1, b"a", 2), MARCA | 7);
        assert_eq!(m.abiertos(), 0);
        assert_eq!(m.clasificador.olvidados, vec![1]);
    }

    #[test]
    fn se_rinde_a_los_12() {
        let mut m = Motor::new(ClasificadorFalso::default());
        for i in 0..11 {
            assert_eq!(m.marca(2, b"nunca", i), 0);
        }
        assert_eq!(m.marca(2, b"nunca", 11), MARCA | 5);
    }

    #[test]
    fn olvida_conexiones_paradas() {
        let mut m = Motor::new(ClasificadorFalso::default());
        m.marca(3, b"a", 0);
        m.limpiar(OLVIDO_MS + 1);
        assert_eq!(m.abiertos(), 0);
        assert_eq!(m.clasificador.olvidados, vec![3]);
    }

    #[test]
    fn version_ndpi_no_soportada() {
        assert!(comprobar_version("3.4").unwrap_err().contains("3.4"));
        assert!(comprobar_version("4.12.0-4000-abc").is_err());
        assert!(comprobar_version("4.14.0-5000-abc").is_ok());
        assert!(comprobar_version("5.0.0").is_ok());
        assert!(comprobar_version("6.0").is_ok());
        assert!(comprobar_version("7.0").is_err());
        assert!(comprobar_version("").is_err());
    }

    #[test]
    fn clave_igual_en_los_dos_sentidos() {
        // IPv4 TCP 192.168.1.20:50000 → 1.1.1.1:443 y la vuelta
        let mut ida = vec![0x45, 0, 0, 40, 0, 0, 0, 0, 64, 6, 0, 0, 192, 168, 1, 20, 1, 1, 1, 1];
        ida.extend([0xc3, 0x50, 0x01, 0xbb]);
        let mut vuelta = vec![0x45, 0, 0, 40, 0, 0, 0, 0, 64, 6, 0, 0, 1, 1, 1, 1, 192, 168, 1, 20];
        vuelta.extend([0x01, 0xbb, 0xc3, 0x50]);
        assert_eq!(clave_flujo(&ida), clave_flujo(&vuelta));
        assert!(clave_flujo(&ida).is_some());
        let mut otra = ida.clone();
        otra[23] = 0x50;
        assert_ne!(clave_flujo(&ida), clave_flujo(&otra));
        assert_eq!(clave_flujo(&[0x45]), None);
    }
}
