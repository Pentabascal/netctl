//! La libndpi del sistema, cargada con `dlopen` (sin enlazarla al compilar). Solo se usan
//! funciones cuya forma es la misma en 4.14, 5.x y 6.x; `ndpi_revision()` se comprueba antes.

use std::collections::HashMap;
use std::ffi::{CStr, c_char, c_int, c_uint, c_void};

use libloading::{Library, Symbol};

use crate::{Clasificador, comprobar_version};

const NOMBRES: [&str; 4] = ["libndpi.so.6", "libndpi.so.5", "libndpi.so.4", "libndpi.so"];
pub const SIN_LIBRERIA: &str = "No está instalada la librería nDPI (paquete libndpi).";

/// `ndpi_protocol` cambia de tamaño entre versiones, pero siempre empieza por
/// `{u16 master, u16 app}` y mide más de 16 bytes (se devuelve por puntero oculto). Un hueco
/// más grande que cualquier versión vale para todas.
#[repr(C)]
#[derive(Clone, Copy)]
struct Protocolo {
    master: u16,
    app: u16,
    resto: [u8; 508],
}

impl Protocolo {
    fn vacio() -> Self {
        Self { master: 0, app: 0, resto: [0; 508] }
    }
}

type FnRevision = unsafe extern "C" fn() -> *const c_char;
// La 6.x pide además el tipo de licencia; a la 4.14 y la 5.x el segundo argumento les sobra.
type FnInit = unsafe extern "C" fn(*mut c_void, c_int) -> *mut c_void;
type FnFinal = unsafe extern "C" fn(*mut c_void) -> c_int;
type FnTamFlujo = unsafe extern "C" fn() -> u32;
type FnProcesar = unsafe extern "C" fn(*mut c_void, *mut c_void, *const u8, u16, u64, *mut c_void) -> Protocolo;
type FnLiberarFlujo = unsafe extern "C" fn(*mut c_void);
type FnNombre = unsafe extern "C" fn(*mut c_void, u16) -> *const c_char;
type FnNum = unsafe extern "C" fn(*mut c_void) -> c_uint;
type FnCategoria = unsafe extern "C" fn(*mut c_void, Protocolo) -> c_int;
type FnNombreCategoria = unsafe extern "C" fn(*mut c_void, c_int) -> *const c_char;
type FnSalir = unsafe extern "C" fn(*mut c_void);

/// Licencia «para un proyecto con ánimo de lucro, solo componentes LGPL» (6.x).
const LICENCIA_LGPL: c_int = 1;

pub struct Ndpi {
    _lib: Library,
    modulo: *mut c_void,
    tam_flujo: usize,
    procesar: FnProcesar,
    liberar: FnLiberarFlujo,
    nombre: FnNombre,
    num: FnNum,
    categoria: Option<FnCategoria>,
    nombre_categoria: Option<FnNombreCategoria>,
    salir: FnSalir,
    pub version: String,
    /// Último resultado por conexión (para rendirse con lo que se sabía) y memoria del flujo.
    flujos: HashMap<u64, (Vec<u64>, u16)>,
}

// El módulo de nDPI solo se usa desde un hilo a la vez (el que lo tiene).
unsafe impl Send for Ndpi {}

/// Una app que se puede elegir en las reglas.
pub struct App {
    pub id: u16,
    pub nombre: String,
    pub categoria: String,
}

unsafe fn sim<T: Copy>(lib: &Library, n: &[u8]) -> Result<T, String> {
    let s: Symbol<T> = unsafe { lib.get(n) }.map_err(|_| {
        format!("La librería nDPI no tiene {}: versión no soportada.", String::from_utf8_lossy(&n[..n.len() - 1]))
    })?;
    Ok(*s)
}

fn texto(p: *const c_char) -> String {
    if p.is_null() { String::new() } else { unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned() }
}

impl Ndpi {
    /// Carga la librería, comprueba la versión y prepara el módulo de detección. El `Err` es
    /// el motivo que se enseña al usuario.
    pub fn cargar() -> Result<Ndpi, String> {
        let lib = NOMBRES.iter().find_map(|n| unsafe { Library::new(n) }.ok()).ok_or(SIN_LIBRERIA)?;
        unsafe {
            let revision: FnRevision = sim(&lib, b"ndpi_revision\0")?;
            let version = texto(revision());
            comprobar_version(&version)?;
            let init: FnInit = sim(&lib, b"ndpi_init_detection_module\0")?;
            let fin: FnFinal = sim(&lib, b"ndpi_finalize_initialization\0")?;
            let tam: FnTamFlujo = sim(&lib, b"ndpi_detection_get_sizeof_ndpi_flow_struct\0")?;
            let num: FnNum = sim(&lib, b"ndpi_get_num_protocols\0").or_else(|_| sim(&lib, b"ndpi_get_num_supported_protocols\0"))?;
            let n = Ndpi {
                procesar: sim(&lib, b"ndpi_detection_process_packet\0")?,
                liberar: sim(&lib, b"ndpi_free_flow_data\0")?,
                nombre: sim(&lib, b"ndpi_get_proto_name\0")?,
                salir: sim(&lib, b"ndpi_exit_detection_module\0")?,
                categoria: sim(&lib, b"ndpi_get_proto_category\0").ok(),
                nombre_categoria: sim(&lib, b"ndpi_category_get_name\0").ok(),
                num,
                tam_flujo: tam() as usize,
                modulo: init(std::ptr::null_mut(), LICENCIA_LGPL),
                version,
                flujos: HashMap::new(),
                _lib: lib,
            };
            if n.modulo.is_null() {
                return Err("nDPI no pudo arrancar (sin memoria).".into());
            }
            if fin(n.modulo) != 0 {
                return Err("nDPI no pudo terminar de arrancar.".into());
            }
            Ok(n)
        }
    }

    /// Apps y protocolos que conoce esta versión de nDPI.
    pub fn apps(&self) -> Vec<App> {
        let total = unsafe { (self.num)(self.modulo) } as u16;
        (1..total)
            .filter_map(|id| {
                let nombre = texto(unsafe { (self.nombre)(self.modulo, id) });
                if nombre.is_empty() || nombre == "Unknown" {
                    return None;
                }
                let categoria = match (self.categoria, self.nombre_categoria) {
                    (Some(c), Some(nc)) => {
                        let mut p = Protocolo::vacio();
                        p.app = id;
                        texto(unsafe { nc(self.modulo, c(self.modulo, p)) })
                    }
                    _ => String::new(),
                };
                Some(App { id, nombre, categoria })
            })
            .collect()
    }
}

impl Clasificador for Ndpi {
    fn paquete(&mut self, flujo: u64, datos: &[u8], ahora_ms: u64) -> Option<u16> {
        let palabras = self.tam_flujo.div_ceil(8);
        let (mem, ultimo) = self.flujos.entry(flujo).or_insert_with(|| (vec![0u64; palabras], 0));
        let largo = datos.len().min(u16::MAX as usize) as u16;
        let p = unsafe { (self.procesar)(self.modulo, mem.as_mut_ptr().cast(), datos.as_ptr(), largo, ahora_ms, std::ptr::null_mut()) };
        if p.master != 0 {
            *ultimo = p.master;
        }
        (p.app != 0).then_some(p.app)
    }

    fn rendirse(&mut self, flujo: u64) -> u16 {
        self.flujos.get(&flujo).map(|(_, u)| *u).unwrap_or(0)
    }

    fn olvidar(&mut self, flujo: u64) {
        if let Some((mut mem, _)) = self.flujos.remove(&flujo) {
            unsafe { (self.liberar)(mem.as_mut_ptr().cast()) };
        }
    }
}

impl Drop for Ndpi {
    fn drop(&mut self) {
        let ids: Vec<u64> = self.flujos.keys().copied().collect();
        for f in ids {
            self.olvidar(f);
        }
        unsafe { (self.salir)(self.modulo) };
    }
}
