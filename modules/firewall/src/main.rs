//! `netctl-mod-firewall`: servicio `netctl-mod-firewall.service` que escucha en
//! `/run/netctl/mod-firewall.sock` (o `NETCTL_SOCKET`).

use std::sync::Arc;

use netctl_mod_firewall::api::{self, Estado};
use netctl_mod_firewall::aplicar::PLAZO;
use netctl_mod_firewall::nft;
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
    tokio::spawn(vigilar_dpi(e.clone()));
    netctl_sdk::servidor::servir("firewall", api::router(e)).await
}

/// Arranca `netctl-dpi cola` mientras haya reglas de App y lo vuelve a arrancar si se cae (el
/// tráfico no se corta mientras tanto: la cola lleva `bypass`). Lo para cuando ya no hacen falta.
async fn vigilar_dpi(e: Arc<Estado>) {
    let mut hijo: Option<std::process::Child> = None;
    let mut t = tokio::time::interval(std::time::Duration::from_secs(10));
    loop {
        t.tick().await;
        if let Some(h) = &mut hijo
            && !matches!(h.try_wait(), Ok(None))
        {
            hijo = None;
        }
        match (e.necesita_dpi(), &mut hijo) {
            (true, None) => {
                // `ct original packets` necesita la contabilidad de conntrack.
                let _ = std::fs::write("/proc/sys/net/netfilter/nf_conntrack_acct", "1");
                match std::process::Command::new(&e.rutas.dpi).args(["cola", &nft::COLA_DPI.to_string()]).spawn() {
                    Ok(h) => hijo = Some(h),
                    Err(x) => eprintln!("No se pudo arrancar netctl-dpi: {x}"),
                }
            }
            (false, Some(h)) => {
                let _ = h.kill();
                let _ = h.wait();
                hijo = None;
            }
            _ => {}
        }
    }
}
