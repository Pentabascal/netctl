//! `netctl-mod-firewall`: servicio `netctl-mod-firewall.service` que escucha en
//! `/run/netctl/mod-firewall.sock` (o `NETCTL_SOCKET`).

use std::sync::Arc;

use netctl_mod_firewall::api::{self, Estado};
use netctl_mod_firewall::aplicar::PLAZO;
use netctl_mod_firewall::sistema::{Rutas, SistemaReal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let e = Estado::nuevo(Rutas::sistema(), Arc::new(SistemaReal), PLAZO)?;
    // Primero las reglas (el servicio arranca antes que la red) y después el socket.
    e.arrancar().await;
    tokio::spawn(e.clone().revisar_siempre());
    netctl_sdk::servidor::servir("firewall", api::router(e)).await
}
