//! Destino por dominio: resuelve los nombres de las reglas y llena los conjuntos `dom{id}_4` y
//! `dom{id}_6` con `nft -f -`, sin recargar la tabla. Cada IP vista se guarda una hora aunque el
//! nombre deje de devolverla (los DNS reparten IPs distintas en cada consulta).

use std::collections::BTreeMap;
use std::net::IpAddr;

use crate::modelo::{Config, Direccion};
use crate::sistema::Sistema;

/// Cuánto se guarda una IP vista (s).
pub const DURA: u64 = 3600;
/// Cada cuánto se vuelve a preguntar al DNS (s).
pub const CADA: u64 = 300;

/// IPs vistas por regla: id → (ip → último instante en que el DNS la dio).
#[derive(Default, Debug, Clone)]
pub struct Cache {
    pub ips: BTreeMap<u64, BTreeMap<IpAddr, u64>>,
    pub ultima: Option<u64>,
    pub avisos: Vec<String>,
}

/// Reglas activas con destino por dominio: (id, nombres).
pub fn reglas_con_dominio(c: &Config) -> Vec<(u64, Vec<String>)> {
    c.reglas
        .iter()
        .filter(|r| r.activa)
        .filter_map(|r| match &r.destino {
            Direccion::Dominio { nombres } => Some((r.id, nombres.clone())),
            _ => None,
        })
        .collect()
}

/// Resuelve (si toca) y vuelve a escribir los conjuntos. Devuelve los avisos actuales.
pub fn refrescar(c: &Config, s: &dyn Sistema, cache: &mut Cache) -> Vec<String> {
    let ahora = s.ahora();
    let reglas = reglas_con_dominio(c);
    if cache.ultima.is_none_or(|u| ahora >= u + CADA) {
        cache.ultima = Some(ahora);
        cache.avisos.clear();
        for (id, nombres) in &reglas {
            let vistas = cache.ips.entry(*id).or_default();
            for n in nombres {
                let ips = s.resolver(n);
                if ips.is_empty() {
                    let a = format!("No se pudo resolver «{n}»; se reintenta en 5 minutos.");
                    if !cache.avisos.contains(&a) {
                        cache.avisos.push(a);
                    }
                }
                for ip in ips {
                    vistas.insert(ip, ahora);
                }
            }
        }
    }
    rellenar(c, s, cache);
    cache.avisos.clone()
}

/// Vuelve a escribir los conjuntos con lo que ya se sabe, sin preguntar al DNS (tras recargar
/// la tabla, que los deja vacíos).
pub fn rellenar(c: &Config, s: &dyn Sistema, cache: &mut Cache) {
    let ahora = s.ahora();
    let reglas = reglas_con_dominio(c);
    cache.ips.retain(|id, _| reglas.iter().any(|(r, _)| r == id));
    for (id, _) in &reglas {
        let vistas = cache.ips.entry(*id).or_default();
        vistas.retain(|_, t| *t + DURA > ahora);
        let (v4, v6): (Vec<IpAddr>, Vec<IpAddr>) = vistas.keys().partition(|ip| ip.is_ipv4());
        // Un conjunto por orden: si uno falla (la tabla aún no lo tiene) no tumba a los demás.
        for (fam, l) in [("4", v4), ("6", v6)] {
            let mut t = format!("flush set inet netctl dom{id}_{fam}\n");
            if !l.is_empty() {
                let e: Vec<String> = l.iter().map(|ip| ip.to_string()).collect();
                t.push_str(&format!("add element inet netctl dom{id}_{fam} {{ {} }}\n", e.join(", ")));
            }
            let _ = s.ejecutar("nft", &["-f", "-"], Some(&t));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modelo::*;
    use crate::pruebas::*;
    use crate::sistema::Falso;

    fn con_dominio(nombres: &[&str]) -> Config {
        let mut c = router();
        let mut r = regla(30, "interna", "externa", Accion::Bloquear);
        r.destino = Direccion::Dominio { nombres: nombres.iter().map(|x| x.to_string()).collect() };
        c.reglas.push(r);
        c
    }

    #[test]
    fn refrescar_anade_elementos() {
        let c = con_dominio(&["tiktok.com"]);
        let s = Falso::new();
        s.responder_dns("tiktok.com", &["203.0.113.9", "2001:db8::9"]);
        let mut cache = Cache::default();
        assert!(refrescar(&c, &s, &mut cache).is_empty());
        let e = s.entradas("nft -f -");
        assert!(e.contains(&"flush set inet netctl dom30_4\nadd element inet netctl dom30_4 { 203.0.113.9 }\n".to_string()), "{e:?}");
        assert!(e.contains(&"flush set inet netctl dom30_6\nadd element inet netctl dom30_6 { 2001:db8::9 }\n".to_string()), "{e:?}");
        // Otra IP más tarde: se guardan las dos hasta que pasa una hora.
        s.responder_dns("tiktok.com", &["203.0.113.10"]);
        *s.reloj.lock().unwrap() += CADA;
        s.limpiar();
        refrescar(&c, &s, &mut cache);
        assert!(s.entradas("nft -f -")[0].contains("{ 203.0.113.9, 203.0.113.10 }"));
        *s.reloj.lock().unwrap() += DURA;
        s.limpiar();
        refrescar(&c, &s, &mut cache);
        assert!(s.entradas("nft -f -")[0].contains("{ 203.0.113.10 }"));
    }

    #[test]
    fn dominio_sin_resolver() {
        let c = con_dominio(&["no-existe.invalid"]);
        let s = Falso::new();
        let mut cache = Cache::default();
        let a = refrescar(&c, &s, &mut cache);
        assert_eq!(a, vec!["No se pudo resolver «no-existe.invalid»; se reintenta en 5 minutos.".to_string()]);
        assert_eq!(s.entradas("nft -f -"), vec!["flush set inet netctl dom30_4\n".to_string(), "flush set inet netctl dom30_6\n".to_string()]);
        // Antes de 5 min no se pregunta otra vez, pero el aviso sigue.
        s.limpiar();
        s.responder_dns("no-existe.invalid", &["198.51.100.1"]);
        assert_eq!(refrescar(&c, &s, &mut cache).len(), 1);
        // y el script de la tabla se genera igual
        assert!(crate::nft::generar(&c, &fijas(), 1000).is_ok());
    }
}
