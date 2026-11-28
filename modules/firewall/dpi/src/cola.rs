//! Lee la cola NFQUEUE, clasifica y devuelve cada paquete aceptado con su marca.

use nfq::{Queue, Verdict};

use crate::{Clasificador, Motor, clave_flujo};

pub fn servir<C: Clasificador>(num: u16, clasificador: C) -> std::io::Result<()> {
    let mut q = Queue::open()?;
    q.bind(num)?;
    // Si la cola se llena, que pase el tráfico en vez de tirarlo.
    q.set_fail_open(num, true)?;
    q.set_recv_conntrack(num, true)?;
    let _ = q.set_recv_enobufs(false);
    let mut m = Motor::new(clasificador);
    let inicio = std::time::Instant::now();
    loop {
        let mut msg = q.recv()?;
        let datos = msg.get_payload();
        let flujo = msg.get_conntrack().map(|c| u64::from(c.get_id()) | 1 << 40).or_else(|| clave_flujo(datos));
        let marca = match flujo {
            Some(f) => m.marca(f, datos, inicio.elapsed().as_millis() as u64),
            None => crate::MARCA,
        };
        if marca != 0 {
            msg.set_nfmark(marca);
        }
        msg.set_verdict(Verdict::Accept);
        q.verdict(msg)?;
    }
}
