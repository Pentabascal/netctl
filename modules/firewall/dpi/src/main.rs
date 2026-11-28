//! `netctl-dpi apps` → JSON con las apps que conoce la libndpi del sistema (o el motivo de que
//! no se pueda).
//! `netctl-dpi cola <n>` → clasifica la cola NFQUEUE `n` hasta que lo paren.

use netctl_dpi::ndpi::Ndpi;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("apps") => {
            let v = match Ndpi::cargar() {
                Ok(n) => serde_json::json!({
                    "disponible": true,
                    "version": n.version,
                    "apps": n.apps().iter().map(|a| serde_json::json!({ "id": a.id, "nombre": a.nombre, "categoria": a.categoria })).collect::<Vec<_>>(),
                }),
                Err(m) => serde_json::json!({ "disponible": false, "motivo": m }),
            };
            println!("{v}");
        }
        Some("cola") => {
            let num: u16 = args.get(1).and_then(|x| x.parse().ok()).unwrap_or(100);
            let n = match Ndpi::cargar() {
                Ok(n) => n,
                Err(m) => {
                    eprintln!("{m}");
                    std::process::exit(2);
                }
            };
            if let Err(e) = netctl_dpi::cola::servir(num, n) {
                eprintln!("Cola {num}: {e}");
                std::process::exit(1);
            }
        }
        _ => {
            eprintln!("uso: netctl-dpi apps | cola <n>");
            std::process::exit(64);
        }
    }
}
