//! Módulo Firewall de netctl: zonas, matriz zona→zona, reglas, grupos y reenvío de puertos,
//! todo en la tabla nftables `inet netctl`. La lógica (modelo → texto nft) es pura y se prueba
//! sin root; lo que toca el sistema pasa por [`sistema::Sistema`].

pub mod api;
pub mod aplicar;
pub mod dominios;
pub mod estado;
pub mod leer;
pub mod modelo;
pub mod nft;
pub mod plantillas;
pub mod registro;
pub mod sistema;
pub mod validar;

/// Convierte un texto cualquiera en algo seguro para comentarios de nft y nombres de fichero.
pub fn nombre_seguro(s: &str) -> String {
    let r: String = s
        .trim()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .take(48)
        .collect();
    if r.is_empty() { "sin_nombre".into() } else { r }
}

#[cfg(test)]
pub(crate) mod pruebas;
