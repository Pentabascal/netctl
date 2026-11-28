//! Destino por país con la base libre «IP to Country Lite» de DB-IP (CC BY 4.0, atribución
//! «IP Geolocation by DB-IP»). Se descarga al usar un país por primera vez y cada mes; una
//! descarga rota o a medias no sustituye a la buena.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read as _;
use std::net::{Ipv4Addr, Ipv6Addr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::modelo::{Config, Direccion};
use crate::sistema::{Rutas, Sistema};

/// Rangos de un país ya unidos: IPv4 e IPv6.
pub type Rangos = (Vec<(u32, u32)>, Vec<(u128, u128)>);
pub type Base = BTreeMap<String, Rangos>;
/// Conjuntos de nft por país usado: código → (elementos v4, elementos v6).
pub type Paises = BTreeMap<String, (Vec<String>, Vec<String>)>;

/// Códigos ISO 3166 alfa-2 (para elegir país aunque aún no haya base).
pub const CODIGOS: &str = "AD AE AF AG AI AL AM AO AQ AR AS AT AU AW AX AZ BA BB BD BE BF BG BH BI BJ BL BM BN BO BQ BR BS BT BV BW BY BZ CA CC CD CF CG CH CI CK CL CM CN CO CR CU CV CW CX CY CZ DE DJ DK DM DO DZ EC EE EG EH ER ES ET FI FJ FK FM FO FR GA GB GD GE GF GG GH GI GL GM GN GP GQ GR GS GT GU GW GY HK HM HN HR HT HU ID IE IL IM IN IO IQ IR IS IT JE JM JO JP KE KG KH KI KM KN KP KR KW KY KZ LA LB LC LI LK LR LS LT LU LV LY MA MC MD ME MF MG MH MK ML MM MN MO MP MQ MR MS MT MU MV MW MX MY MZ NA NC NE NF NG NI NL NO NP NR NU NZ OM PA PE PF PG PH PK PL PM PN PR PS PT PW PY QA RE RO RS RU RW SA SB SC SD SE SG SH SI SJ SK SL SM SN SO SR SS ST SV SX SY SZ TC TD TF TG TH TJ TK TL TM TN TO TR TT TV TW TZ UA UG UM US UY UZ VA VC VE VG VI VN VU WF WS YE YT ZA ZM ZW";

pub const AVISO_DESCARGANDO: &str = "Descargando la base de países…; la regla se aplicará al terminar.";

pub fn url(anio: i64, mes: u32) -> String {
    format!("https://download.db-ip.com/free/dbip-country-lite-{anio:04}-{mes:02}.csv.gz")
}

pub fn es_codigo(c: &str) -> bool {
    c.len() == 2 && c.bytes().all(|b| b.is_ascii_uppercase())
}

fn limpio(x: &str) -> &str {
    x.trim().trim_matches('"')
}

/// Lee el CSV entero (`inicio,fin,país`). Cualquier línea rara lo invalida todo.
pub fn analizar(csv: &str) -> Result<Base, String> {
    let mut m: Base = BTreeMap::new();
    let mut n = 0usize;
    for (i, l) in csv.lines().enumerate() {
        if l.trim().is_empty() {
            continue;
        }
        let p: Vec<&str> = l.split(',').map(limpio).collect();
        let error = || format!("La línea {} de la base de países no se entiende.", i + 1);
        if p.len() != 3 || !es_codigo(p[2]) {
            return Err(error());
        }
        let e = m.entry(p[2].to_string()).or_default();
        if let (Ok(a), Ok(b)) = (p[0].parse::<Ipv4Addr>(), p[1].parse::<Ipv4Addr>()) {
            let (a, b) = (u32::from(a), u32::from(b));
            if a > b {
                return Err(error());
            }
            match e.0.last_mut() {
                Some(u) if u.1.checked_add(1) == Some(a) => u.1 = b,
                _ => e.0.push((a, b)),
            }
        } else if let (Ok(a), Ok(b)) = (p[0].parse::<Ipv6Addr>(), p[1].parse::<Ipv6Addr>()) {
            let (a, b) = (u128::from(a), u128::from(b));
            if a > b {
                return Err(error());
            }
            match e.1.last_mut() {
                Some(u) if u.1.checked_add(1) == Some(a) => u.1 = b,
                _ => e.1.push((a, b)),
            }
        } else {
            return Err(error());
        }
        n += 1;
    }
    if n == 0 {
        return Err("La base de países está vacía.".into());
    }
    // nft rechaza elementos solapados en un conjunto con intervalos: se ordenan y se juntan.
    for (v4, v6) in m.values_mut() {
        *v4 = juntar(std::mem::take(v4));
        *v6 = juntar(std::mem::take(v6));
    }
    Ok(m)
}

/// Ordena y une rangos que se solapan o se tocan.
fn juntar<T: Ord + Copy + num_siguiente::Siguiente>(mut v: Vec<(T, T)>) -> Vec<(T, T)> {
    v.sort();
    let mut r: Vec<(T, T)> = Vec::with_capacity(v.len());
    for (a, b) in v {
        match r.last_mut() {
            Some(u) if a <= u.1 || u.1.siguiente() == Some(a) => u.1 = u.1.max(b),
            _ => r.push((a, b)),
        }
    }
    r
}

mod num_siguiente {
    pub trait Siguiente: Sized {
        fn siguiente(self) -> Option<Self>;
    }
    impl Siguiente for u32 {
        fn siguiente(self) -> Option<Self> {
            self.checked_add(1)
        }
    }
    impl Siguiente for u128 {
        fn siguiente(self) -> Option<Self> {
            self.checked_add(1)
        }
    }
}

/// Elementos de nft de un país.
pub fn textos(r: &Rangos) -> (Vec<String>, Vec<String>) {
    let t4 = r.0.iter().map(|(a, b)| if a == b { Ipv4Addr::from(*a).to_string() } else { format!("{}-{}", Ipv4Addr::from(*a), Ipv4Addr::from(*b)) });
    let t6 = r.1.iter().map(|(a, b)| if a == b { Ipv6Addr::from(*a).to_string() } else { format!("{}-{}", Ipv6Addr::from(*a), Ipv6Addr::from(*b)) });
    (t4.collect(), t6.collect())
}

fn dir(r: &Rutas) -> PathBuf {
    r.datos.join("geo")
}
fn actual(r: &Rutas) -> PathBuf {
    dir(r).join("actual.csv.gz")
}

/// Año y mes (UTC) de un instante UNIX.
pub fn anio_mes(t: u64) -> (i64, u32, u32) {
    // Días desde 1970 → fecha civil (algoritmo de H. Hinnant).
    let z = (t / 86400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

fn gunzip(b: &[u8]) -> Result<String, String> {
    let mut s = String::new();
    flate2::read::GzDecoder::new(b).read_to_string(&mut s).map_err(|e| format!("La base de países llegó rota: {e}"))?;
    Ok(s)
}

/// Descarga la base del mes (o la del anterior si aún no está), la comprueba entera y solo
/// entonces sustituye a la que había. Devuelve «AAAA-MM».
pub fn actualizar(r: &Rutas, s: &dyn Sistema) -> Result<String, String> {
    let (a, m, _) = anio_mes(s.ahora());
    let (pa, pm) = if m == 1 { (a - 1, 12) } else { (a, m - 1) };
    std::fs::create_dir_all(dir(r)).map_err(|e| e.to_string())?;
    let nuevo = dir(r).join("nuevo.csv.gz");
    let mut ultimo = String::new();
    for (a, m) in [(a, m), (pa, pm)] {
        let _ = std::fs::remove_file(&nuevo);
        match s.descargar(&url(a, m), &nuevo).and_then(|_| std::fs::read(&nuevo).map_err(|e| e.to_string())).and_then(|b| gunzip(&b)).and_then(|t| analizar(&t)) {
            Ok(_) => {
                std::fs::rename(&nuevo, actual(r)).map_err(|e| e.to_string())?;
                let f = format!("{a:04}-{m:02}");
                std::fs::write(dir(r).join("fecha"), &f).map_err(|e| e.to_string())?;
                return Ok(f);
            }
            Err(e) => ultimo = e,
        }
    }
    let _ = std::fs::remove_file(&nuevo);
    Err(ultimo)
}

/// «AAAA-MM» de la base que hay, si hay.
pub fn fecha(r: &Rutas) -> Option<String> {
    actual(r).exists().then(|| std::fs::read_to_string(dir(r).join("fecha")).unwrap_or_default().trim().to_string())
}

/// ¿Toca descargar? Si no hay base, o si es del mes pasado y ya es día 2 o más.
pub fn toca_actualizar(r: &Rutas, ahora: u64) -> bool {
    let (a, m, d) = anio_mes(ahora);
    match fecha(r) {
        None => true,
        Some(f) => f != format!("{a:04}-{m:02}") && d >= 2,
    }
}

type Cargadas = BTreeMap<PathBuf, (std::time::SystemTime, Arc<Base>)>;
static CARGADAS: Mutex<Cargadas> = Mutex::new(BTreeMap::new());

/// La base en memoria (se vuelve a leer si cambia el fichero).
pub fn base(r: &Rutas) -> Option<Arc<Base>> {
    let ruta = actual(r);
    let mt = std::fs::metadata(&ruta).and_then(|m| m.modified()).ok()?;
    let mut c = CARGADAS.lock().unwrap();
    if let Some((t, b)) = c.get(&ruta)
        && *t == mt
    {
        return Some(b.clone());
    }
    let b = Arc::new(analizar(&gunzip(&std::fs::read(&ruta).ok()?).ok()?).ok()?);
    c.insert(ruta, (mt, b.clone()));
    Some(b)
}

/// Países que usan las reglas activas.
pub fn usados(c: &Config) -> BTreeSet<String> {
    c.reglas
        .iter()
        .filter(|r| r.activa)
        .flat_map(|r| [&r.origen, &r.destino])
        .filter_map(|d| match d {
            Direccion::Pais { codigos } => Some(codigos.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Conjuntos de los países usados (vacío si aún no hay base).
pub fn paises(c: &Config, r: &Rutas) -> Paises {
    let u = usados(c);
    if u.is_empty() {
        return Paises::new();
    }
    let Some(b) = base(r) else { return Paises::new() };
    u.into_iter().map(|cc| (cc.clone(), b.get(&cc).map(textos).unwrap_or_default())).collect()
}

/// Aviso si alguna regla usa países y aún no hay base.
pub fn avisos(c: &Config, r: &Rutas) -> Vec<String> {
    if !usados(c).is_empty() && fecha(r).is_none() { vec![AVISO_DESCARGANDO.into()] } else { vec![] }
}

/// Ruta del fichero ya descargado (para la demo y las pruebas).
pub fn ruta_actual(r: &Rutas) -> PathBuf {
    actual(r)
}

/// Comprime `texto` como lo sirve DB-IP (para pruebas y la demo).
pub fn gz(texto: &str) -> Vec<u8> {
    use std::io::Write as _;
    let mut e = flate2::write::GzEncoder::new(vec![], flate2::Compression::fast());
    e.write_all(texto.as_bytes()).unwrap();
    e.finish().unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modelo::*;
    use crate::pruebas::*;
    use crate::sistema::Falso;

    const MINI: &str = include_str!("../tests/datos/dbip-mini.csv");
    // 2026-11-28 12:00 UTC
    const AHORA: u64 = 1_795_867_200;

    #[test]
    fn analiza_y_une_rangos() {
        let b = analizar(MINI).unwrap();
        let (v4, v6) = textos(&b["ES"]);
        assert_eq!(v4, vec!["2.136.0.0-2.139.255.255", "5.0.0.0"]);
        assert_eq!(v6, vec!["2a01:c500::-2a01:c51f:ffff:ffff:ffff:ffff:ffff:ffff"]);
        assert_eq!(textos(&b["FR"]).0, vec!["2.140.0.0-2.140.0.255"]);
        assert!(analizar("1.0.0.0,1.0.0.255,es").is_err());
        // desordenados y solapados: se juntan (nft no acepta solapes)
        let b = analizar("10.0.0.50,10.0.0.80,XX\n10.0.0.0,10.0.0.60,XX\n10.0.0.81,10.0.0.90,XX\n10.0.1.0,10.0.1.0,XX").unwrap();
        assert_eq!(textos(&b["XX"]).0, vec!["10.0.0.0-10.0.0.90", "10.0.1.0"]);
        assert!(analizar("").is_err());
    }

    #[test]
    fn url_del_mes() {
        assert_eq!(anio_mes(AHORA), (2026, 11, 28));
        assert_eq!(anio_mes(0), (1970, 1, 1));
        assert_eq!(url(2026, 3), "https://download.db-ip.com/free/dbip-country-lite-2026-03.csv.gz");
    }

    fn montar() -> (tempfile::TempDir, Rutas, Falso) {
        let d = tempfile::tempdir().unwrap();
        let r = Rutas::en(d.path());
        let s = Falso::new();
        *s.reloj.lock().unwrap() = AHORA;
        (d, r, s)
    }

    #[test]
    fn csv_roto_conserva_anterior() {
        let (_d, r, s) = montar();
        s.responder_descarga(&url(2026, 11), gz(MINI));
        assert_eq!(actualizar(&r, &s).unwrap(), "2026-11");
        let antes = std::fs::read(ruta_actual(&r)).unwrap();
        // gz cortado a la mitad, y un CSV con una línea mala
        let mut medio = gz(MINI);
        medio.truncate(medio.len() / 2);
        s.responder_descarga(&url(2026, 11), medio);
        s.responder_descarga(&url(2026, 10), gz("1.0.0.0,1.0.0.255,AU\nbasura"));
        assert!(actualizar(&r, &s).is_err());
        assert_eq!(std::fs::read(ruta_actual(&r)).unwrap(), antes);
        assert_eq!(fecha(&r).as_deref(), Some("2026-11"));
        assert!(!dir(&r).join("nuevo.csv.gz").exists());
    }

    #[test]
    fn mes_anterior_si_aun_no_esta() {
        let (_d, r, s) = montar();
        s.responder_descarga(&url(2026, 10), gz(MINI));
        assert_eq!(actualizar(&r, &s).unwrap(), "2026-10");
        assert!(toca_actualizar(&r, AHORA));
    }

    #[test]
    fn pais_sin_base_aviso() {
        let (_d, r, _s) = montar();
        let mut c = router();
        let mut x = regla(31, "externa", "gateway", Accion::Bloquear);
        x.origen = Direccion::Pais { codigos: vec!["CN".into()] };
        c.reglas.push(x);
        assert_eq!(avisos(&c, &r), vec![AVISO_DESCARGANDO.to_string()]);
        assert!(paises(&c, &r).is_empty());
        // la regla no sale (no hay conjunto), pero el resto sí
        let t = crate::nft::generar(&c, &fijas(), 1000).unwrap();
        assert!(!t.contains("pais_"), "{t}");
        assert!(t.contains("chain input"));
    }

    #[test]
    fn pais_genera_conjunto() {
        let (_d, r, s) = montar();
        s.responder_descarga(&url(2026, 11), gz(MINI));
        actualizar(&r, &s).unwrap();
        let mut c = router();
        let mut x = regla(31, "interna", "externa", Accion::Bloquear);
        x.destino = Direccion::Pais { codigos: vec!["ES".into(), "CN".into()] };
        c.reglas.push(x);
        assert!(avisos(&c, &r).is_empty());
        let mut f = fijas();
        f.paises = paises(&c, &r);
        let t = crate::nft::generar(&c, &f, 1000).unwrap();
        assert!(t.contains("set pais_es_4 { type ipv4_addr; flags interval; elements = { 2.136.0.0-2.139.255.255, 5.0.0.0 } }"), "{t}");
        assert!(t.contains("set pais_es_6 { type ipv6_addr; flags interval;"), "{t}");
        assert!(t.contains("ip daddr @pais_es_4 tcp dport 80 counter drop"), "{t}");
        assert!(t.contains("ip daddr @pais_cn_4 tcp dport 80 counter drop"), "{t}");
        assert!(!t.contains("pais_cn_6"), "CN no tiene IPv6 en la mini: {t}");
    }
}
