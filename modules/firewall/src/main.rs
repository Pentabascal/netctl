//! `netctl-mod-firewall`: servicio `netctl-mod-firewall.service` que escucha en
//! `/run/netctl/mod-firewall.sock` (o `NETCTL_SOCKET`).

use std::sync::Arc;

use netctl_mod_firewall::api::{self, Estado};
use netctl_mod_firewall::aplicar::PLAZO;
use netctl_mod_firewall::sistema::{Rutas, SistemaReal};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `netctl-mod-firewall desinstalar [--restaurar]`: lo usa el núcleo si el servicio no
    // responde al aviso de desinstalación.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) == Some("desinstalar") {
        let s = SistemaReal;
        let r = Rutas::sistema();
        let m = if args.iter().any(|a| a == "--restaurar") {
            netctl_mod_firewall::aplicar::restaurar_anterior(&s, &r)
        } else {
            netctl_mod_firewall::aplicar::quitar_tabla(&s);
            vec!["Tabla del firewall quitada.".into()]
        };
        for x in m {
            println!("{x}");
        }
        return Ok(());
    }
    let e = Estado::nuevo(Rutas::sistema(), Arc::new(SistemaReal), PLAZO)?;
    // Primero las reglas (el servicio arranca antes que la red) y después el socket.
    e.arrancar().await;
    tokio::spawn(e.clone().revisar_siempre());
    netctl_sdk::servidor::servir("firewall", api::router(e)).await
}
