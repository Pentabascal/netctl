//! `netctl-core serve` | `netctl-core admin set-password|reset-password|quitar-totp`

use std::io::IsTerminal;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use axum_server::tls_rustls::RustlsConfig;
use ipnet::IpNet;
use netctl_core::alcance;
use netctl_core::api::{self, Estado};
use netctl_core::auth::Almacen;
use netctl_core::config::{Alcance, Config, RUTA_CONFIG};
use netctl_core::modulos::{self, Gestor, Rutas, SystemdReal, catalogo};
use netctl_core::{ficheros, proxy, tls};

const AYUDA: &str = "Uso:
  netctl-core serve                    Arranca el panel
  netctl-core admin set-password       Cambia la contraseña del administrador
  netctl-core admin reset-password     Pone una contraseña aleatoria y la muestra una vez
  netctl-core admin quitar-totp        Quita el doble factor y vuelve al acceso solo LAN

La configuración se lee de /etc/netctl/core.toml (o de NETCTL_CONFIG).";

fn ruta_config() -> PathBuf {
    std::env::var_os("NETCTL_CONFIG")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| RUTA_CONFIG.into())
}

fn almacen(c: &Config) -> Result<Almacen> {
    Almacen::abrir(&c.datos.join("admin.json"))
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let ruta = ruta_config();
    match args.as_slice() {
        ["serve"] => tokio::runtime::Runtime::new()?.block_on(servir(&ruta)),
        ["admin", "set-password"] => {
            let c = Config::cargar(&ruta)?;
            let p = leer_password()?;
            almacen(&c)?.fijar_password(&p)?;
            println!("Contraseña cambiada. Se han cerrado las sesiones abiertas.");
            Ok(())
        }
        ["admin", "reset-password"] => {
            let c = Config::cargar(&ruta)?;
            let p = password_aleatoria();
            almacen(&c)?.fijar_password(&p)?;
            println!("Contraseña nueva del usuario «admin» (no se volverá a mostrar): {p}");
            Ok(())
        }
        ["admin", "quitar-totp"] => {
            let mut c = Config::cargar(&ruta)?;
            almacen(&c)?.quitar_totp()?;
            if c.alcance != Alcance::SoloLan {
                c.alcance = Alcance::SoloLan;
                c.guardar(&ruta)?;
            }
            println!(
                "Doble factor quitado; el panel queda en «solo LAN/VPN». Reinicia el servicio: systemctl restart netctl"
            );
            Ok(())
        }
        ["-h" | "--help" | "help"] | [] => {
            println!("{AYUDA}");
            Ok(())
        }
        _ => bail!("Orden no reconocida.\n\n{AYUDA}"),
    }
}

fn password_aleatoria() -> String {
    use base64::Engine;
    let mut b = [0u8; 15];
    getrandom::getrandom(&mut b).expect("sin fuente de aleatoriedad");
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(b)
}

fn leer_password() -> Result<String> {
    let p = if std::io::stdin().is_terminal() {
        let a = rpassword::prompt_password("Contraseña nueva: ")?;
        let b = rpassword::prompt_password("Repítela: ")?;
        if a != b {
            bail!("Las contraseñas no coinciden");
        }
        a
    } else {
        let mut l = String::new();
        std::io::stdin().read_line(&mut l)?;
        l.trim_end_matches(['\r', '\n']).to_string()
    };
    if p.chars().count() < 10 {
        bail!("La contraseña debe tener al menos 10 caracteres");
    }
    Ok(p)
}

/// Redes WireGuard con una caché de 10 s (se consultan en cada petición).
fn redes_wg_cacheadas() -> Arc<dyn Fn() -> Vec<IpNet> + Send + Sync> {
    let cache: Mutex<Option<(Instant, Vec<IpNet>)>> = Mutex::new(None);
    Arc::new(move || {
        let mut c = cache.lock().unwrap();
        match &*c {
            Some((t, v)) if t.elapsed() < Duration::from_secs(10) => v.clone(),
            _ => {
                let v = alcance::redes_wg(&alcance::interfaces());
                *c = Some((Instant::now(), v.clone()));
                v
            }
        }
    })
}

fn enlazar(ip: IpAddr, puerto: u16) -> Result<std::net::TcpListener> {
    let l = std::net::TcpListener::bind(SocketAddr::new(ip, puerto))
        .with_context(|| format!("No se pudo escuchar en {ip}:{puerto}"))?;
    l.set_nonblocking(true)?;
    Ok(l)
}

async fn servir(ruta: &Path) -> Result<()> {
    tls::instalar_proveedor();
    let config = Config::cargar(ruta)?;
    ficheros::crear_dir(&config.datos, 0o700)?;
    let auth = almacen(&config)?;
    if !auth.hay_password() {
        eprintln!(
            "Aviso: no hay contraseña de administrador; ejecuta «netctl-core admin reset-password»"
        );
    }
    let host = std::fs::read_to_string("/etc/hostname")
        .map(|h| h.trim().to_string())
        .unwrap_or_default();
    let (cert, clave) = tls::asegurar_certificado(&config.tls, &[host])?;
    let tls_cfg = RustlsConfig::from_pem_file(&cert, &clave)
        .await
        .context("Certificado TLS no válido")?;
    let puerto = config.puerto;
    let estado = Arc::new(Estado::new(
        config,
        ruta.to_path_buf(),
        auth,
        redes_wg_cacheadas(),
    ));
    let mut rutas = Rutas::sistema(&estado.config.read().unwrap().datos);
    // Solo para desarrollo (pruebas visuales sin root): carpeta de módulos y de sockets.
    if let Some(d) = std::env::var_os("NETCTL_DIR_MODULOS").filter(|v| !v.is_empty()) {
        rutas.modulos = d.into();
    }
    if let Some(d) = std::env::var_os("NETCTL_DIR_RUN").filter(|v| !v.is_empty()) {
        rutas.run = d.into();
    }
    let gestor = Arc::new(Gestor::new(
        rutas,
        Arc::new(SystemdReal),
        catalogo::URL_BASE,
        catalogo::clave_publica(),
        netctl_sdk::distro::detectar(),
    ));
    if gestor.clave.is_none() {
        eprintln!(
            "Aviso: el núcleo no tiene clave pública de firma; el catálogo de módulos no funcionará"
        );
    }
    let protegidas = modulos::rutas(gestor.clone()).merge(proxy::rutas(gestor));
    let app = api::router(estado.clone(), protegidas);
    let mut rx = estado.alcance_tx.subscribe();

    loop {
        let alc = *rx.borrow_and_update();
        let dirs = alcance::direcciones_escucha(alc, &alcance::interfaces());
        if alc == Alcance::SoloLan && dirs == [IpAddr::V4(Ipv4Addr::LOCALHOST)] {
            eprintln!(
                "Aviso: no hay interfaces privadas ni WireGuard; el panel solo escucha en 127.0.0.1"
            );
        }
        let handle = axum_server::Handle::new();
        let mut tareas = Vec::new();
        for ip in &dirs {
            let escucha = match enlazar(*ip, puerto) {
                Ok(l) => l,
                // Sin IPv6 en el sistema: se escucha solo en IPv4.
                Err(_) if ip.is_unspecified() && ip.is_ipv6() => {
                    enlazar(IpAddr::V4(Ipv4Addr::UNSPECIFIED), puerto)?
                }
                Err(e) => {
                    eprintln!("{e:#}");
                    continue;
                }
            };
            eprintln!("netctl escuchando en https://{}", escucha.local_addr()?);
            let srv = axum_server::from_tcp_rustls(escucha, tls_cfg.clone()).handle(handle.clone());
            let svc = app
                .clone()
                .into_make_service_with_connect_info::<SocketAddr>();
            tareas.push(tokio::spawn(async move { srv.serve(svc).await }));
        }
        if tareas.is_empty() {
            bail!("No se pudo escuchar en ninguna dirección");
        }
        // Se vuelve a enlazar si cambia el alcance o las interfaces (p. ej. aparece wg0).
        loop {
            tokio::select! {
                r = rx.changed() => { r?; break; }
                _ = tokio::time::sleep(Duration::from_secs(30)) => {
                    if alcance::direcciones_escucha(*rx.borrow(), &alcance::interfaces()) != dirs { break; }
                }
            }
        }
        handle.graceful_shutdown(Some(Duration::from_secs(5)));
        for t in tareas {
            let _ = t.await;
        }
    }
}
