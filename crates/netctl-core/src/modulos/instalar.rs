//! Instalar, actualizar y desinstalar módulos.

use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use minisign_verify::PublicKey;
use netctl_sdk::distro::Familia;
use netctl_sdk::manifiesto::Manifiesto;
use semver::Version;
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

use super::catalogo::{EntradaCatalogo, descargar, verificar};
use super::{Rutas, Systemd, id_valido};

const LIMITE_PAQUETE: usize = 256 * 1024 * 1024;
const LIMITE_EXTRAIDO: u64 = 512 * 1024 * 1024;
const LIMITE_FIRMA: usize = 64 * 1024;

#[derive(Debug)]
pub enum ErrorInstalar {
    Descarga(String),
    Firma,
    Sha256,
    NucleoAntiguo {
        requerido: Version,
        actual: Version,
    },
    /// El `.tar.gz` o su `manifest.toml` no son válidos.
    Paquete(String),
    /// No se pudieron instalar los paquetes del sistema.
    Dependencias(String),
    Sistema(String),
    /// El módulo no respondió a `/salud` a tiempo.
    Salud,
}

impl std::fmt::Display for ErrorInstalar {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Descarga(e) => write!(f, "No se pudo descargar el módulo: {e}"),
            Self::Firma => write!(f, "La firma del módulo no es válida; no se instala"),
            Self::Sha256 => write!(
                f,
                "El módulo descargado no coincide con el catálogo (sha256); no se instala"
            ),
            Self::NucleoAntiguo { requerido, actual } => write!(
                f,
                "Este módulo necesita el núcleo {requerido} o posterior y tienes el {actual}. Actualiza primero el núcleo"
            ),
            Self::Paquete(e) => write!(f, "El paquete del módulo no es válido: {e}"),
            Self::Dependencias(e) => write!(f, "{e}"),
            Self::Sistema(e) => write!(f, "{e}"),
            Self::Salud => write!(
                f,
                "El módulo no respondió tras arrancar; se ha dejado como estaba"
            ),
        }
    }
}

impl std::error::Error for ErrorInstalar {}

/// Lo que necesita la instalación del sistema (inyectable en tests).
#[derive(Clone)]
pub struct Entorno {
    pub rutas: Rutas,
    pub systemd: Arc<dyn Systemd>,
    pub cliente: reqwest::Client,
    /// Tiempo máximo para que el módulo responda a `/salud` (15 s).
    pub espera_salud: Duration,
}

pub fn version_nucleo() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("versión del núcleo")
}

/// Ruta relativa segura: solo componentes normales (sin `/`, `..` ni prefijos).
fn ruta_segura(p: &Path) -> Option<PathBuf> {
    let mut r = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(n) => r.push(n),
            Component::CurDir => {}
            _ => return None,
        }
    }
    Some(r)
}

/// Extrae un `.tar.gz` en `destino` (que se vacía antes). Solo admite ficheros normales y
/// carpetas con rutas relativas; rechaza rutas absolutas, `..`, enlaces y ficheros especiales.
pub fn extraer(tar_gz: &[u8], destino: &Path) -> Result<(), String> {
    let r = extraer_en(tar_gz, destino);
    if r.is_err() {
        let _ = std::fs::remove_dir_all(destino);
    }
    r
}

fn extraer_en(tar_gz: &[u8], destino: &Path) -> Result<(), String> {
    let io = |e: std::io::Error| e.to_string();
    match std::fs::remove_dir_all(destino) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.to_string()),
        _ => {}
    }
    crate::ficheros::crear_dir(destino, 0o755).map_err(|e| format!("{e:#}"))?;
    let mut ar = tar::Archive::new(flate2::read::GzDecoder::new(tar_gz));
    let mut total: u64 = 0;
    for entrada in ar.entries().map_err(io)? {
        let mut e = entrada.map_err(io)?;
        let ruta = e.path().map_err(io)?.into_owned();
        let Some(rel) = ruta_segura(&ruta) else {
            return Err(format!(
                "ruta no permitida en el paquete: {}",
                ruta.display()
            ));
        };
        let tipo = e.header().entry_type();
        if rel.as_os_str().is_empty() {
            if tipo.is_dir() {
                continue;
            }
            return Err("entrada sin nombre en el paquete".into());
        }
        let final_ = destino.join(&rel);
        if tipo.is_dir() {
            std::fs::create_dir_all(&final_).map_err(io)?;
            continue;
        }
        if !(tipo.is_file() || tipo == tar::EntryType::Continuous) {
            return Err(format!(
                "el paquete contiene un enlace o fichero especial: {}",
                ruta.display()
            ));
        }
        total = total.saturating_add(e.header().size().map_err(io)?);
        if total > LIMITE_EXTRAIDO {
            return Err("el paquete extraído es demasiado grande".into());
        }
        if let Some(padre) = final_.parent() {
            std::fs::create_dir_all(padre).map_err(io)?;
        }
        let ejecutable = e.header().mode().map_err(io)? & 0o111 != 0;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(if ejecutable { 0o755 } else { 0o644 })
            .custom_flags(libc::O_NOFOLLOW)
            .open(&final_)
            .map_err(|err| format!("{}: {err}", rel.display()))?;
        let copiados = std::io::copy(&mut (&mut e).take(LIMITE_EXTRAIDO), &mut f).map_err(io)?;
        if copiados != e.header().size().map_err(io)? {
            return Err(format!("{}: tamaño inesperado", rel.display()));
        }
    }
    Ok(())
}

/// Unidad systemd del módulo.
pub fn unidad(rutas: &Rutas, m: &Manifiesto) -> String {
    let nombre: String = m.nombre.chars().filter(|c| !c.is_control()).collect();
    let id = &m.id;
    format!(
        "[Unit]\n\
         Description=netctl: módulo {nombre}\n\
         After=network-online.target\n\
         Wants=network-online.target\n\
         \n\
         [Service]\n\
         ExecStart={bin}\n\
         Environment=NETCTL_SOCKET={socket}\n\
         Restart=on-failure\n\
         RestartSec=2\n\
         MemoryMax=512M\n\
         CPUWeight=50\n\
         RuntimeDirectory=netctl\n\
         RuntimeDirectoryPreserve=yes\n\
         StateDirectory=netctl/{id}\n\
         StateDirectoryMode=0700\n\
         \n\
         [Install]\n\
         WantedBy=multi-user.target\n",
        bin = rutas.binario(id).display(),
        socket = rutas.socket(id).display(),
    )
}

/// Instala o actualiza el módulo de `entrada`. Si ya estaba instalado, la versión anterior se
/// guarda en `<id>.anterior/` y se recupera si la nueva no responde a `/salud`.
pub async fn instalar(
    env: &Entorno,
    entrada: &EntradaCatalogo,
    clave: &PublicKey,
    familia: Option<Familia>,
) -> Result<(), ErrorInstalar> {
    let id = entrada.id.clone();
    if !id_valido(&id) {
        return Err(ErrorInstalar::Paquete(format!(
            "id de módulo no válido: {id}"
        )));
    }
    let actual = version_nucleo();
    if entrada.nucleo_minimo > actual {
        return Err(ErrorInstalar::NucleoAntiguo {
            requerido: entrada.nucleo_minimo.clone(),
            actual,
        });
    }
    // 1. Descargar y verificar firma y sha256.
    let tar = descargar(&env.cliente, &entrada.url, LIMITE_PAQUETE)
        .await
        .map_err(ErrorInstalar::Descarga)?;
    let firma = descargar(
        &env.cliente,
        &format!("{}.minisig", entrada.url),
        LIMITE_FIRMA,
    )
    .await
    .map_err(ErrorInstalar::Descarga)?;
    if !verificar(clave, &tar, &firma) {
        return Err(ErrorInstalar::Firma);
    }
    let sha = Sha256::digest(&tar);
    let esperado = entrada.sha256.trim().to_ascii_lowercase();
    let calculado: String = sha.iter().map(|b| format!("{b:02x}")).collect();
    if !bool::from(calculado.as_bytes().ct_eq(esperado.as_bytes())) {
        return Err(ErrorInstalar::Sha256);
    }

    // 2. Extraer a `<id>.nuevo/` y comprobar el manifiesto.
    let rutas = env.rutas.clone();
    let nuevo = rutas.modulos.join(format!("{id}.nuevo"));
    let manifiesto = {
        let (nuevo, id) = (nuevo.clone(), id.clone());
        let bin = rutas
            .binario(&id)
            .strip_prefix(rutas.dir_modulo(&id))
            .unwrap()
            .to_path_buf();
        tokio::task::spawn_blocking(move || -> Result<Manifiesto, ErrorInstalar> {
            std::fs::create_dir_all(nuevo.parent().unwrap())
                .map_err(|e| ErrorInstalar::Sistema(e.to_string()))?;
            extraer(&tar, &nuevo).map_err(ErrorInstalar::Paquete)?;
            let comprobar = || {
                let texto = std::fs::read_to_string(nuevo.join("manifest.toml"))
                    .map_err(|_| ErrorInstalar::Paquete("falta manifest.toml".into()))?;
                let m = Manifiesto::leer(&texto)
                    .map_err(|e| ErrorInstalar::Paquete(format!("{e:#}")))?;
                if m.id != id {
                    return Err(ErrorInstalar::Paquete(format!(
                        "el manifiesto es de «{}», no de «{id}»",
                        m.id
                    )));
                }
                if m.nucleo_minimo > actual {
                    return Err(ErrorInstalar::NucleoAntiguo {
                        requerido: m.nucleo_minimo.clone(),
                        actual,
                    });
                }
                if !nuevo.join(&bin).is_file() {
                    return Err(ErrorInstalar::Paquete(format!("falta {}", bin.display())));
                }
                Ok(m)
            };
            let r = comprobar();
            if r.is_err() {
                let _ = std::fs::remove_dir_all(&nuevo);
            }
            r
        })
        .await
        .map_err(|e| ErrorInstalar::Sistema(e.to_string()))??
    };

    // 3. Paquetes del sistema.
    let paquetes = match familia {
        Some(Familia::Debian) => manifiesto.paquetes.debian.clone(),
        Some(Familia::Suse) => manifiesto.paquetes.suse.clone(),
        Some(Familia::Rhel) => manifiesto.paquetes.rhel.clone(),
        None => {
            let p = &manifiesto.paquetes;
            if !(p.debian.is_empty() && p.suse.is_empty() && p.rhel.is_empty()) {
                let _ = std::fs::remove_dir_all(&nuevo);
                return Err(ErrorInstalar::Dependencias(
                    "No se reconoce la distribución y el módulo necesita paquetes del sistema"
                        .into(),
                ));
            }
            vec![]
        }
    };
    if let Some(f) = familia.filter(|_| !paquetes.is_empty()) {
        let sd = env.systemd.clone();
        let r = tokio::task::spawn_blocking(move || sd.instalar_paquetes(f, &paquetes)).await;
        if let Err(e) = r.map_err(anyhow::Error::from).and_then(|r| r) {
            let _ = std::fs::remove_dir_all(&nuevo);
            return Err(ErrorInstalar::Dependencias(format!("{e:#}")));
        }
    }

    // 4. Cambiar de versión, crear la unidad y arrancar.
    let dir = rutas.dir_modulo(&id);
    let anterior = rutas.modulos.join(format!("{id}.anterior"));
    let actualizando = dir.exists();
    let sistema = |e: std::io::Error| ErrorInstalar::Sistema(e.to_string());
    if actualizando {
        let _ = std::fs::remove_dir_all(&anterior);
        std::fs::rename(&dir, &anterior).map_err(sistema)?;
    }
    if let Err(e) = std::fs::rename(&nuevo, &dir) {
        if actualizando {
            let _ = std::fs::rename(&anterior, &dir);
        }
        return Err(sistema(e));
    }
    let unidad_txt = unidad(&rutas, &manifiesto);
    let arrancar = async {
        std::fs::create_dir_all(&rutas.unidades).map_err(sistema)?;
        crate::ficheros::escribir_atomico(&rutas.unidad(&id), unidad_txt.as_bytes(), 0o644)
            .map_err(|e| ErrorInstalar::Sistema(format!("{e:#}")))?;
        systemctl(env, {
            let u = nombre_unidad(&id);
            move |sd| {
                sd.daemon_reload()?;
                sd.enable_now(&u)?;
                if actualizando { sd.restart(&u) } else { Ok(()) }
            }
        })
        .await?;
        if esperar_salud(&rutas.socket(&id), env.espera_salud).await {
            Ok(())
        } else {
            Err(ErrorInstalar::Salud)
        }
    };
    let Err(fallo) = arrancar.await else {
        return Ok(());
    };

    // 5. No arrancó: volver a como estaba.
    eprintln!("Instalación de {id} fallida ({fallo}); se deshace");
    let _ = std::fs::remove_dir_all(&dir);
    let u = nombre_unidad(&id);
    if actualizando {
        let _ = std::fs::rename(&anterior, &dir);
        let _ = systemctl(env, move |sd| sd.restart(&u)).await;
    } else {
        let _ = std::fs::remove_file(rutas.unidad(&id));
        let _ = systemctl(env, move |sd| {
            let r = sd.disable_now(&u);
            sd.daemon_reload()?;
            r
        })
        .await;
    }
    Err(fallo)
}

fn nombre_unidad(id: &str) -> String {
    super::nombre_unidad(id)
}

/// Ejecuta órdenes de systemd fuera del runtime async.
async fn systemctl(
    env: &Entorno,
    f: impl FnOnce(&dyn Systemd) -> anyhow::Result<()> + Send + 'static,
) -> Result<(), ErrorInstalar> {
    let sd = env.systemd.clone();
    tokio::task::spawn_blocking(move || f(sd.as_ref()))
        .await
        .map_err(|e| ErrorInstalar::Sistema(e.to_string()))?
        .map_err(|e| ErrorInstalar::Sistema(format!("{e:#}")))
}

/// Espera a que `GET /salud` responda 2xx en `socket` antes de `plazo`.
pub async fn esperar_salud(socket: &Path, plazo: Duration) -> bool {
    let fin = Instant::now() + plazo;
    loop {
        let req = axum::http::Request::get("/salud")
            .header("host", "localhost")
            .body(axum::body::Body::empty())
            .unwrap();
        let resto = fin.saturating_duration_since(Instant::now());
        if let Ok(Ok(r)) = tokio::time::timeout(
            resto.min(Duration::from_secs(2)),
            crate::proxy::enviar(socket, req),
        )
        .await
            && r.status().is_success()
        {
            return true;
        }
        if Instant::now() >= fin {
            return false;
        }
        tokio::time::sleep(
            Duration::from_millis(200).min(fin.saturating_duration_since(Instant::now())),
        )
        .await;
    }
}

/// Para y quita el módulo. Sus datos (`<datos>/<id>`) se conservan salvo `borrar_datos`.
pub async fn desinstalar(env: &Entorno, id: &str, borrar_datos: bool) -> Result<(), ErrorInstalar> {
    if !id_valido(id) {
        return Err(ErrorInstalar::Paquete(format!(
            "id de módulo no válido: {id}"
        )));
    }
    let rutas = &env.rutas;
    let u = nombre_unidad(id);
    // Si la unidad ya no existe, disable falla: no impide seguir quitando ficheros.
    if let Err(e) = systemctl(env, move |sd| sd.disable_now(&u)).await {
        eprintln!("Aviso al parar el módulo {id}: {e}");
    }
    let sistema = |e: std::io::Error| ErrorInstalar::Sistema(e.to_string());
    let borrar = |p: PathBuf| match std::fs::remove_dir_all(&p) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(sistema(e)),
        _ => Ok(()),
    };
    match std::fs::remove_file(rutas.unidad(id)) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(sistema(e)),
        _ => {}
    }
    systemctl(env, |sd| sd.daemon_reload()).await?;
    borrar(rutas.dir_modulo(id))?;
    borrar(rutas.modulos.join(format!("{id}.anterior")))?;
    borrar(rutas.modulos.join(format!("{id}.nuevo")))?;
    if borrar_datos {
        borrar(rutas.datos.join(id))?;
    }
    let _ = std::fs::remove_file(rutas.socket(id));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modulos::pruebas::*;

    async fn preparar(
        k: &Claves,
        version: &str,
        nucleo_minimo: &str,
    ) -> (
        tempfile::TempDir,
        Entorno,
        Arc<SystemdDoble>,
        EntradaCatalogo,
    ) {
        let dir = tempfile::tempdir().unwrap();
        let (env, doble) = entorno(dir.path());
        let tar = tar_modulo("prueba", version, nucleo_minimo);
        let sha = hex(&Sha256::digest(&tar));
        let firma = k.firmar(&tar);
        let base = servidor_ficheros(vec![("/m.tar.gz", tar), ("/m.tar.gz.minisig", firma)]).await;
        let e = entrada(
            "prueba",
            version,
            nucleo_minimo,
            &format!("{base}m.tar.gz"),
            &sha,
        );
        (dir, env, doble, e)
    }

    #[tokio::test]
    async fn instala_y_arranca() {
        let k = Claves::nuevas();
        let (_d, env, doble, e) = preparar(&k, "0.1.0", "0.1.0").await;
        let _srv = modulo_falso(&env.rutas.socket("prueba")).await;
        instalar(&env, &e, &k.publica, Some(Familia::Debian))
            .await
            .unwrap();
        let dir = env.rutas.dir_modulo("prueba");
        assert!(dir.join("bin/netctl-mod-prueba").is_file());
        assert!(dir.join("ui/index.js").is_file());
        let u = std::fs::read_to_string(env.rutas.unidad("prueba")).unwrap();
        for l in [
            &format!("ExecStart={}", dir.join("bin/netctl-mod-prueba").display()),
            "Restart=on-failure",
            "MemoryMax=512M",
            "CPUWeight=50",
            "RuntimeDirectory=netctl",
            "StateDirectory=netctl/prueba",
        ] {
            assert!(u.lines().any(|x| x == l), "falta «{l}» en\n{u}");
        }
        assert_eq!(
            doble.llamadas(),
            ["daemon-reload", "enable --now netctl-mod-prueba.service"]
        );
    }

    #[tokio::test]
    async fn firma_mala_rechaza() {
        let k = Claves::nuevas();
        let (_d, env, doble, e) = preparar(&k, "0.1.0", "0.1.0").await;
        let otra = Claves::nuevas();
        let r = instalar(&env, &e, &otra.publica, Some(Familia::Debian)).await;
        assert!(matches!(r, Err(ErrorInstalar::Firma)), "{r:?}");
        assert!(!env.rutas.dir_modulo("prueba").exists());
        assert!(doble.llamadas().is_empty());
    }

    #[tokio::test]
    async fn sha_distinto_rechaza() {
        let k = Claves::nuevas();
        let (_d, env, doble, mut e) = preparar(&k, "0.1.0", "0.1.0").await;
        e.sha256 = "ab".repeat(32);
        let r = instalar(&env, &e, &k.publica, Some(Familia::Debian)).await;
        assert!(matches!(r, Err(ErrorInstalar::Sha256)), "{r:?}");
        assert!(!env.rutas.dir_modulo("prueba").exists());
        assert!(doble.llamadas().is_empty());
    }

    #[tokio::test]
    async fn nucleo_antiguo_bloquea() {
        let k = Claves::nuevas();
        // El catálogo dice que vale, pero el manifiesto del paquete pide más.
        let (_d, env, doble, mut e) = preparar(&k, "0.1.0", "99.0.0").await;
        e.nucleo_minimo = Version::new(0, 1, 0);
        let r = instalar(&env, &e, &k.publica, Some(Familia::Debian)).await;
        assert!(
            matches!(r, Err(ErrorInstalar::NucleoAntiguo { .. })),
            "{r:?}"
        );
        assert!(!env.rutas.dir_modulo("prueba").exists());
        assert!(doble.llamadas().is_empty());
        // Y si lo dice el catálogo, ni se descarga.
        e.nucleo_minimo = Version::new(99, 0, 0);
        e.url = format!("{}nada", url_cerrada().await);
        let r = instalar(&env, &e, &k.publica, Some(Familia::Debian)).await;
        assert!(
            matches!(r, Err(ErrorInstalar::NucleoAntiguo { .. })),
            "{r:?}"
        );
    }

    #[tokio::test]
    async fn salud_falla_restaura_anterior() {
        let k = Claves::nuevas();
        let (_d, env, doble, e) = preparar(&k, "0.2.0", "0.1.0").await;
        // Versión 0.1.0 ya instalada.
        let actual = env.rutas.dir_modulo("prueba");
        extraer(&tar_modulo("prueba", "0.1.0", "0.1.0"), &actual).unwrap();
        // Sin módulo escuchando en el socket: /salud nunca responde.
        let r = instalar(&env, &e, &k.publica, Some(Familia::Debian)).await;
        assert!(matches!(r, Err(ErrorInstalar::Salud)), "{r:?}");
        let m = std::fs::read_to_string(actual.join("manifest.toml")).unwrap();
        assert!(m.contains("version = \"0.1.0\""), "{m}");
        assert!(!env.rutas.modulos.join("prueba.nuevo").exists());
        let ll = doble.llamadas();
        assert!(
            ll.iter().filter(|l| l.starts_with("restart")).count() >= 2,
            "{ll:?}"
        );
    }

    #[tokio::test]
    async fn salud_falla_en_instalacion_nueva_limpia() {
        let k = Claves::nuevas();
        let (_d, env, doble, e) = preparar(&k, "0.1.0", "0.1.0").await;
        let r = instalar(&env, &e, &k.publica, Some(Familia::Debian)).await;
        assert!(matches!(r, Err(ErrorInstalar::Salud)), "{r:?}");
        assert!(!env.rutas.dir_modulo("prueba").exists());
        assert!(!env.rutas.unidad("prueba").exists());
        assert!(
            doble
                .llamadas()
                .contains(&"disable --now netctl-mod-prueba.service".to_string())
        );
    }

    #[tokio::test]
    async fn desinstalar_conserva_datos() {
        let k = Claves::nuevas();
        let (_d, env, doble, e) = preparar(&k, "0.1.0", "0.1.0").await;
        let _srv = modulo_falso(&env.rutas.socket("prueba")).await;
        instalar(&env, &e, &k.publica, None).await.unwrap();
        let datos = env.rutas.datos.join("prueba");
        std::fs::create_dir_all(&datos).unwrap();
        desinstalar(&env, "prueba", false).await.unwrap();
        assert!(!env.rutas.dir_modulo("prueba").exists());
        assert!(!env.rutas.unidad("prueba").exists());
        assert!(datos.exists());
        assert!(
            doble
                .llamadas()
                .contains(&"disable --now netctl-mod-prueba.service".to_string())
        );
        assert!(desinstalar(&env, "../etc", true).await.is_err());
    }

    fn tar_con(cabecera: impl FnOnce(&mut tar::Header), datos: &[u8]) -> Vec<u8> {
        let mut h = tar::Header::new_gnu();
        h.set_size(datos.len() as u64);
        h.set_mode(0o644);
        h.set_entry_type(tar::EntryType::Regular);
        cabecera(&mut h);
        h.set_cksum();
        let gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        let mut b = tar::Builder::new(gz);
        b.append(&h, datos).unwrap();
        b.into_inner().unwrap().finish().unwrap()
    }

    fn nombre_crudo(h: &mut tar::Header, n: &[u8]) {
        let campo = &mut h.as_old_mut().name;
        campo.fill(0);
        campo[..n.len()].copy_from_slice(n);
    }

    #[test]
    fn tar_con_ruta_maliciosa_rechaza() {
        let raiz = tempfile::tempdir().unwrap();
        let destino = raiz.path().join("modulo");
        let casos: Vec<(&str, Vec<u8>)> = vec![
            (
                "relativa con ..",
                tar_con(|h| nombre_crudo(h, b"../fuera"), b"x"),
            ),
            (
                ".. en medio",
                tar_con(|h| nombre_crudo(h, b"bin/../../fuera"), b"x"),
            ),
            (
                "absoluta",
                tar_con(|h| nombre_crudo(h, b"/tmp/netctl-fuera"), b"x"),
            ),
            (
                "enlace simbólico",
                tar_con(
                    |h| {
                        nombre_crudo(h, b"bin");
                        h.set_entry_type(tar::EntryType::Symlink);
                        h.set_size(0);
                        h.set_link_name("/etc").unwrap();
                    },
                    b"",
                ),
            ),
            (
                "enlace duro",
                tar_con(
                    |h| {
                        nombre_crudo(h, b"passwd");
                        h.set_entry_type(tar::EntryType::Link);
                        h.set_size(0);
                        h.set_link_name("../../etc/passwd").unwrap();
                    },
                    b"",
                ),
            ),
            (
                "dispositivo",
                tar_con(
                    |h| {
                        nombre_crudo(h, b"dev");
                        h.set_entry_type(tar::EntryType::Char);
                        h.set_size(0);
                    },
                    b"",
                ),
            ),
        ];
        for (caso, tar) in casos {
            assert!(extraer(&tar, &destino).is_err(), "no rechaza: {caso}");
            assert!(!raiz.path().join("fuera").exists(), "{caso}");
            assert!(!Path::new("/tmp/netctl-fuera").exists(), "{caso}");
            assert!(!destino.join("bin").is_symlink(), "{caso}");
        }
        // Uno bueno sí se extrae, con permisos sin setuid.
        let bueno = tar_con(
            |h| {
                nombre_crudo(h, b"./bin/programa");
                h.set_mode(0o4755);
            },
            b"#!",
        );
        extraer(&bueno, &destino).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let modo = std::fs::metadata(destino.join("bin/programa"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(modo & 0o7777, 0o755);
    }
}
