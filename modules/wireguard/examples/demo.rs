//! Demo para pruebas visuales (no se empaqueta): el módulo con un `Sistema` falso que no toca
//! nada y un `wg show dump` simulado con clientes conectados, desconectados y que nunca han
//! conectado. Todo vive dentro de `<raíz>`.
//!
//!   cargo run -p netctl-mod-wireguard --example demo -- <socket> <raíz> [--vacio] [--caido]
//!
//! `--vacio`: sin proyecto (muestra el asistente). `--caido`: `wg show` falla (interfaz caída).

use std::path::PathBuf;
use std::sync::Arc;

use netctl_mod_wireguard::Rutas;
use netctl_mod_wireguard::api::{self, Estado};
use netctl_mod_wireguard::aplicar::Sistema;
use netctl_mod_wireguard::estado::{Almacen, Documento};
use wg_modelo::Proyecto;

struct Falso {
    datos: PathBuf,
    inicio: u64,
    caido: bool,
}

impl Sistema for Falso {
    fn ejecutar(
        &self,
        programa: &str,
        args: &[&str],
        _entrada: Option<&str>,
    ) -> Result<String, String> {
        if programa == "wg" && args.first() == Some(&"show") {
            if self.caido {
                return Err("Unable to access interface: No such device".into());
            }
            return Ok(self.dump());
        }
        Ok(String::new())
    }
}

impl Falso {
    fn dump(&self) -> String {
        let ahora = self.ahora();
        let t = ahora.saturating_sub(self.inicio);
        let mut o = String::from("cHJpdg==\tcHVi\t51820\toff\n");
        let Ok(Some(d)) = Almacen::new(&self.datos).cargar() else {
            return o;
        };
        for (i, c) in d
            .proyecto
            .clientes
            .iter()
            .enumerate()
            .filter(|(_, c)| c.activo)
        {
            let n = i as u64;
            let ip_pub = format!(
                "{}.{}.{}.{}:{}",
                80 + n * 7,
                30 + n * 11 % 200,
                12 + n * 5,
                40 + n * 3,
                51000 + n * 13
            );
            let (hs, rx, tx, ep) = match i % 7 {
                // Conectados: saludo reciente y tráfico que crece
                0 | 1 | 2 | 4 => {
                    let rx = 1_200_000_000 / (n + 1)
                        + t * (180_000 + n * 40_000)
                        + (t * t * 3000) % 900_000;
                    let tx = 300_000_000 / (n + 1) + t * (25_000 + n * 9_000);
                    (ahora - (8 + n * 23) % 110, rx, tx, ip_pub)
                }
                3 => (ahora - 3 * 3600 - 420, 52_000_000, 8_400_000, ip_pub),
                6 => (ahora - 2 * 86400 - 3000, 640_000_000, 41_000_000, ip_pub),
                _ => (0, 0, 0, "(none)".into()),
            };
            o.push_str(&format!(
                "{}\t(none)\t{ep}\t{}/32\t{hs}\t{rx}\t{tx}\t25\n",
                c.clave_publica, c.ip
            ));
        }
        o
    }
}

fn proyecto() -> Proyecto {
    let mut p = Proyecto::default();
    p.servidor.endpoint = "vpn.ejemplo.es".into();
    p.servidor.iface_salida = "enp3s0".into();
    p.anadir_subred("LAN de casa", "192.168.1.0/24");
    p.anadir_subred("Servidores", "192.168.50.0/24");
    let clientes = [
        ("Portátil del trabajo", false, true),
        ("Móvil de Ana", true, true),
        ("Tablet del salón", false, false),
        ("PC de la oficina", false, true),
        ("Móvil de Luis", true, true),
        ("Raspberry remota", false, false),
        ("Portátil de invitados", true, false),
        ("Consola", false, false),
    ];
    for (nombre, internet, toda) in clientes {
        let i = p.anadir_cliente();
        let c = &mut p.clientes[i];
        c.nombre = nombre.into();
        c.internet = internet;
        c.toda_intranet = toda;
        if !toda {
            c.subredes = vec![2];
        }
    }
    p.clientes[7].activo = false;
    p
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(socket), Some(raiz)) = (args.first(), args.get(1)) else {
        anyhow::bail!("Uso: demo <socket> <raíz> [--vacio] [--caido]");
    };
    let raiz = PathBuf::from(raiz);
    let rutas = Rutas::en(&raiz);
    std::fs::create_dir_all(rutas.proc_route.parent().unwrap())?;
    std::fs::write(
        &rutas.proc_route,
        "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n\
         enp3s0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
         enp3s0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n",
    )?;
    for i in ["lo", "enp3s0", "wlan0"] {
        std::fs::create_dir_all(rutas.sys_net.join(i))?;
    }
    let almacen = Almacen::new(&rutas.datos);
    if !args.iter().any(|a| a == "--vacio")
        && almacen.cargar().map_err(anyhow::Error::msg)?.is_none()
    {
        almacen
            .guardar(&Documento {
                revision: 1,
                proyecto: proyecto(),
            })
            .map_err(anyhow::Error::msg)?;
    }
    let falso = Falso {
        datos: rutas.datos.clone(),
        inicio: std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_secs(),
        caido: args.iter().any(|a| a == "--caido"),
    };
    let e = Estado::nuevo(rutas, Arc::new(falso))?;
    e.arrancar().await;
    tokio::spawn(e.clone().sondear_siempre());
    eprintln!("demo de WireGuard en {socket}");
    netctl_sdk::servidor::servir_en(std::path::Path::new(socket), api::router(e)).await
}
