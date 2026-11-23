//! Cambios que se deshacen solos si no se confirman a tiempo (p. ej. reglas que pueden
//! dejar al administrador fuera del panel).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::Result;

pub type IdCambio = u64;

type Restaurar = Box<dyn FnOnce() + Send>;

#[derive(Default)]
struct Estado {
    sig: IdCambio,
    pendientes: HashMap<IdCambio, Restaurar>,
}

/// Cambios aplicados a la espera de confirmación. Necesita un runtime tokio.
#[derive(Clone, Default)]
pub struct Pendiente {
    estado: Arc<Mutex<Estado>>,
}

impl Pendiente {
    pub fn new() -> Self {
        Self::default()
    }

    /// Ejecuta `nuevo`; si falla, ejecuta `restaurar` y devuelve el error. Si va bien,
    /// `restaurar` se ejecutará cuando pase `plazo` salvo que antes se llame a `confirmar`.
    pub fn aplicar(
        &self,
        nuevo: impl FnOnce() -> Result<()>,
        restaurar: impl FnOnce() + Send + 'static,
        plazo: Duration,
    ) -> Result<IdCambio> {
        if let Err(e) = nuevo() {
            restaurar();
            return Err(e);
        }
        let id = {
            let mut e = self.estado.lock().unwrap();
            e.sig += 1;
            let id = e.sig;
            e.pendientes.insert(id, Box::new(restaurar));
            id
        };
        let estado = self.estado.clone();
        tokio::spawn(async move {
            tokio::time::sleep(plazo).await;
            let r = estado.lock().unwrap().pendientes.remove(&id);
            if let Some(restaurar) = r {
                let _ = tokio::task::spawn_blocking(restaurar).await;
            }
        });
        Ok(id)
    }

    /// Da por bueno el cambio `id`. Devuelve `false` si no existe o ya se revirtió.
    pub fn confirmar(&self, id: IdCambio) -> bool {
        self.estado.lock().unwrap().pendientes.remove(&id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    fn marca() -> (Arc<AtomicBool>, impl FnOnce() + Send + 'static) {
        let f = Arc::new(AtomicBool::new(false));
        let g = f.clone();
        (f, move || g.store(true, Ordering::SeqCst))
    }

    #[tokio::test]
    async fn confirmar_revierte() {
        let p = Pendiente::new();
        let (restaurado, restaurar) = marca();
        let id = p.aplicar(|| Ok(()), restaurar, Duration::from_millis(50)).unwrap();
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(restaurado.load(Ordering::SeqCst));
        assert!(!p.confirmar(id), "ya no se puede confirmar tras el plazo");
    }

    #[tokio::test]
    async fn confirmar_a_tiempo() {
        let p = Pendiente::new();
        let (restaurado, restaurar) = marca();
        let id = p.aplicar(|| Ok(()), restaurar, Duration::from_millis(50)).unwrap();
        assert!(p.confirmar(id));
        assert!(!p.confirmar(id), "solo se confirma una vez");
        tokio::time::sleep(Duration::from_millis(250)).await;
        assert!(!restaurado.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn fallo_al_aplicar_restaura() {
        let p = Pendiente::new();
        let (restaurado, restaurar) = marca();
        let r = p.aplicar(|| anyhow::bail!("nft falló"), restaurar, Duration::from_secs(60));
        assert!(r.is_err());
        assert!(restaurado.load(Ordering::SeqCst));
    }
}
