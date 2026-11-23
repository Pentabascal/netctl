//! `netctl-mod-wireguard`: servicio `netctl-mod-wireguard.service` que escucha en
//! `/run/netctl/mod-wireguard.sock` (o `NETCTL_SOCKET`).

use std::sync::Arc;

use netctl_mod_wireguard::Rutas;
use netctl_mod_wireguard::api::{self, Estado};
use netctl_mod_wireguard::aplicar::SistemaReal;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let e = Estado::nuevo(Rutas::sistema(), Arc::new(SistemaReal))?;
    e.arrancar().await;
    tokio::spawn(e.clone().sondear_siempre());
    netctl_sdk::servidor::servir("wireguard", api::router(e)).await
}
