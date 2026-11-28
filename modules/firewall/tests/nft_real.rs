//! Carga el script de ejemplo en un espacio de red aislado (`unshare -rn`) para comprobar que
//! el kernel lo acepta. Se salta si no hay `nft` o `unshare` sin privilegios.
use std::process::Command;

#[test]
fn kernel_acepta_ejemplo() {
    carga(&[]);
}

/// Reglas con todos los campos de la fase 2.5 (red, MAC, puertos de origen, protocolo por
/// número, estado y horarios).
#[test]
fn campos_nuevos_cargan() {
    carga(&["nuevos"]);
}

/// La zona Hotspot con tarjeta (sin tarjeta ya la lleva el ejemplo base).
#[test]
fn hotspot_con_tarjeta_carga() {
    carga(&["hotspot"]);
}

#[test]
fn hotspot_sin_tarjetas_carga() {
    carga(&[]);
}

/// Reglas NAT propias: masquerade, SNAT con grupo de origen y DNAT a IPv4 e IPv6.
#[test]
fn nat_propio_carga() {
    carga(&["nat"]);
}

/// Conjuntos por dominio y el `flush`/`add element` que los llena.
#[test]
fn dominio_carga() {
    carga(&["dominio"]);
}

/// Conjuntos por país (rangos v4 y v6) como origen.
#[test]
fn pais_carga() {
    carga(&["pais"]);
}

/// Cadena `dpi` con la cola y reglas que comparan `ct mark`.
#[test]
fn app_carga() {
    carga(&["app"]);
}

fn carga(args: &[&str]) {
    let script = Command::new(env!("CARGO"))
        .args(["run", "-q", "-p", "netctl-mod-firewall", "--example", "volcar_nft", "--"])
        .args(args)
        .output()
        .expect("cargo run");
    assert!(script.status.success(), "{}", String::from_utf8_lossy(&script.stderr));
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("ej.nft");
    std::fs::write(&f, &script.stdout).unwrap();
    let r = Command::new("unshare").args(["-rn", "nft", "-f"]).arg(&f).output();
    match r {
        Ok(o) if o.status.success() => {}
        Ok(o) => {
            let err = String::from_utf8_lossy(&o.stderr);
            if err.contains("Operation not permitted") || err.contains("No such file") {
                eprintln!("saltado: {err}");
                return;
            }
            panic!("nft rechaza el script:\n{err}");
        }
        Err(e) => eprintln!("saltado: {e}"),
    }
}
