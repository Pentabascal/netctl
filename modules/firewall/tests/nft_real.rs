//! Carga el script de ejemplo en un espacio de red aislado (`unshare -rn`) para comprobar que
//! el kernel lo acepta. Se salta si no hay `nft` o `unshare` sin privilegios.
use std::process::Command;

#[test]
fn kernel_acepta_ejemplo() {
    let script = Command::new(env!("CARGO"))
        .args(["run", "-q", "-p", "netctl-mod-firewall", "--example", "volcar_nft"])
        .output()
        .expect("cargo run");
    assert!(script.status.success());
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
