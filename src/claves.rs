//! Generación de claves WireGuard (Curve25519), compatible con `wg genkey` / `wg pubkey` / `wg genpsk`.

use base64::{Engine, engine::general_purpose::STANDARD};

fn aleatorio32() -> [u8; 32] {
    let mut b = [0u8; 32];
    getrandom::getrandom(&mut b).expect("no se pudo obtener aleatoriedad del sistema");
    b
}

/// Devuelve (privada, pública) en base64.
pub fn generar_par() -> (String, String) {
    let mut k = aleatorio32();
    // "clamping" igual que hace `wg genkey`
    k[0] &= 248;
    k[31] &= 127;
    k[31] |= 64;
    let p = x25519_dalek::x25519(k, x25519_dalek::X25519_BASEPOINT_BYTES);
    (STANDARD.encode(k), STANDARD.encode(p))
}

/// Calcula la clave pública a partir de una privada en base64.
pub fn publica_de(privada: &str) -> Option<String> {
    let bytes = STANDARD.decode(privada.trim()).ok()?;
    let k: [u8; 32] = bytes.try_into().ok()?;
    Some(STANDARD.encode(x25519_dalek::x25519(k, x25519_dalek::X25519_BASEPOINT_BYTES)))
}

pub fn generar_psk() -> String {
    STANDARD.encode(aleatorio32())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn par_coherente() {
        let (privada, publica) = generar_par();
        assert_eq!(publica_de(&privada).unwrap(), publica);
        assert_eq!(STANDARD.decode(&publica).unwrap().len(), 32);
    }

    #[test]
    fn vector_rfc7748() {
        // RFC 7748 §6.1: clave privada de Alice -> pública de Alice
        let hex = |s: &str| -> Vec<u8> {
            (0..s.len()).step_by(2).map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap()).collect()
        };
        let a = STANDARD.encode(hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a"));
        let esperado = STANDARD.encode(hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"));
        assert_eq!(publica_de(&a).unwrap(), esperado);
    }
}
