//! Llevar la configuración al sistema con vuelta atrás automática si no se confirma, y el
//! traspaso desde firewalld/ufw.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use netctl_sdk::confirmar::{IdCambio, Pendiente};

use crate::estado::Almacen;
use crate::leer::Vecino;
use crate::modelo::{Config, Direccion, Fijas};
use crate::nft;
use crate::registro::{Entrada, EstadoCambio, Registro};
use crate::sistema::{Rutas, Sistema};

pub const PLAZO: Duration = Duration::from_secs(60);

/// Cambio a la espera de confirmación.
#[derive(Clone, Debug, PartialEq)]
pub struct EnEspera {
    pub id: IdCambio,
    /// Instante UNIX en que se deshace solo.
    pub vence: u64,
}

pub struct Aplicador {
    pub sistema: Arc<dyn Sistema>,
    pub rutas: Rutas,
    pub almacen: Arc<Almacen>,
    pub registro: Arc<Registro>,
    pendiente: Pendiente,
    plazo: Duration,
    en_espera: Arc<Mutex<Option<EnEspera>>>,
}

impl Aplicador {
    pub fn new(sistema: Arc<dyn Sistema>, rutas: Rutas, plazo: Duration) -> Self {
        let almacen = Arc::new(Almacen::new(&rutas.datos));
        let registro = Arc::new(Registro::new(&rutas.datos));
        Self { sistema, rutas, almacen, registro, pendiente: Pendiente::new(), plazo, en_espera: Arc::new(Mutex::new(None)) }
    }

    /// Aplica `nueva`. Con `confirmar`, se deshace sola (vuelve a la última confirmada) si no se
    /// llama a [`Aplicador::confirmar`] antes del plazo. Necesita un runtime tokio.
    pub fn aplicar(&self, nueva: &Config, fijas: &Fijas, usuario: &str, resumen: &str, confirmar: bool) -> Result<Option<EnEspera>, String> {
        let ahora = self.sistema.ahora();
        let script = nft::generar(nueva, fijas, ahora).map_err(|e| e.join(" "))?;
        let anotar = |estado, detalle: Option<String>| {
            self.registro.anotar(Entrada { fecha: ahora, usuario: usuario.into(), resumen: resumen.into(), estado, detalle });
        };
        if let Err(e) = self.sistema.ejecutar("nft", &["-f", "-"], Some(&script)) {
            anotar(EstadoCambio::Error, Some(e.clone()));
            // Si había un cambio en espera, el sistema sigue con él: no se toca.
            return Err(traducir_nft(&e));
        }
        if !confirmar {
            // Un cambio sin confirmación (caducidad, IP de un equipo) parte de la confirmada:
            // si había uno en espera, se descarta su vuelta atrás y queda este.
            self.cancelar_espera();
            self.almacen.guardar_confirmada(nueva)?;
            self.almacen.borrar_pendiente();
            anotar(EstadoCambio::Aplicado, None);
            return Ok(None);
        }
        self.cancelar_espera();
        self.almacen.guardar_pendiente(nueva)?;
        anotar(EstadoCambio::Pendiente, None);

        // Vuelta atrás: a la última configuración confirmada, o sin tabla si no había ninguna.
        let anterior = self.almacen.cargar_confirmada()?;
        let restaurar_script = match &anterior {
            Some(c) => Some(nft::generar(c, fijas, ahora).map_err(|e| e.join(" "))?),
            None => None,
        };
        let sistema = self.sistema.clone();
        let rutas = self.rutas.clone();
        let almacen = self.almacen.clone();
        let registro = self.registro.clone();
        let en_espera = self.en_espera.clone();
        let usuario2 = usuario.to_string();
        let resumen2 = resumen.to_string();
        let id_celda: Arc<Mutex<Option<IdCambio>>> = Arc::new(Mutex::new(None));
        let id_celda2 = id_celda.clone();
        let restaurar = move || {
            let r = match &restaurar_script {
                Some(t) => sistema.ejecutar("nft", &["-f", "-"], Some(t)).map(|_| ()),
                // Nunca se confirmó nada: se vuelve al firewall de antes (o a ninguno).
                None => {
                    restaurar_anterior(&*sistema, &rutas);
                    Ok(())
                }
            };
            almacen.borrar_pendiente();
            let mut e = en_espera.lock().unwrap();
            if e.as_ref().map(|x| Some(x.id)) == Some(*id_celda2.lock().unwrap()) {
                *e = None;
            }
            registro.anotar(Entrada {
                fecha: sistema.ahora(),
                usuario: usuario2,
                resumen: format!("{resumen2} (no se confirmó en {} s)", PLAZO.as_secs()),
                estado: EstadoCambio::Revertido,
                detalle: r.err(),
            });
        };
        let id = self.pendiente.aplicar(|| Ok(()), restaurar, self.plazo).map_err(|e| e.to_string())?;
        *id_celda.lock().unwrap() = Some(id);
        let e = EnEspera { id, vence: ahora + self.plazo.as_secs() };
        *self.en_espera.lock().unwrap() = Some(e.clone());
        Ok(Some(e))
    }

    /// Quita la vuelta atrás del cambio en espera (sin deshacerlo).
    fn cancelar_espera(&self) {
        if let Some(e) = self.en_espera.lock().unwrap().take() {
            self.pendiente.confirmar(e.id);
        }
    }

    pub fn confirmar(&self, id: IdCambio, usuario: &str) -> bool {
        let mut e = self.en_espera.lock().unwrap();
        if e.as_ref().map(|x| x.id) != Some(id) || !self.pendiente.confirmar(id) {
            return false;
        }
        *e = None;
        drop(e);
        match self.almacen.cargar_pendiente() {
            Ok(Some(c)) => {
                if let Err(err) = self.almacen.guardar_confirmada(&c) {
                    eprintln!("{err}");
                }
                self.almacen.borrar_pendiente();
            }
            _ => eprintln!("Confirmado un cambio sin pendiente.json"),
        }
        self.registro.anotar(Entrada {
            fecha: self.sistema.ahora(),
            usuario: usuario.into(),
            resumen: "Cambios confirmados".into(),
            estado: EstadoCambio::Confirmado,
            detalle: None,
        });
        true
    }

    pub fn en_espera(&self) -> Option<EnEspera> {
        self.en_espera.lock().unwrap().clone()
    }

    /// Al arrancar el servicio: descarta lo pendiente y carga la última configuración confirmada.
    pub fn arrancar(&self, fijas_de: impl Fn(&Config) -> Fijas) -> Result<Option<Config>, String> {
        let habia_pendiente = self.almacen.cargar_pendiente().ok().flatten().is_some();
        self.almacen.borrar_pendiente();
        let Some(c) = self.almacen.cargar_confirmada()? else {
            // Nunca se confirmó nada: si quedó un cambio a medias (p. ej. el asistente y un
            // reinicio antes de confirmar), se quita y vuelve el firewall de antes.
            if habia_pendiente {
                restaurar_anterior(&*self.sistema, &self.rutas);
            }
            return Ok(None);
        };
        let script = nft::generar(&c, &fijas_de(&c), self.sistema.ahora()).map_err(|e| e.join(" "))?;
        self.sistema.ejecutar("nft", &["-f", "-"], Some(&script)).map_err(|e| traducir_nft(&e))?;
        Ok(Some(c))
    }
}

/// Mensaje en castellano para un fallo de `nft -f`.
pub fn traducir_nft(e: &str) -> String {
    format!("El sistema no aceptó las reglas del firewall y no se ha cambiado nada. Detalle técnico (nft): {e}")
}

/// Otro firewall activo en la máquina (se desactivará al configurar este).
pub fn otro_firewall(s: &dyn Sistema) -> Option<&'static str> {
    if s.ejecutar("systemctl", &["is-active", "firewalld"], None).is_ok_and(|o| o.trim() == "active") {
        Some("firewalld")
    } else if s.ejecutar("ufw", &["status"], None).is_ok_and(|o| o.contains("Status: active")) {
        Some("ufw")
    } else {
        None
    }
}

/// Al instalar: si firewalld o ufw están activos, guarda su estado y los desactiva. Devuelve
/// mensajes para el registro.
pub fn traspaso_inicial(s: &dyn Sistema, r: &Rutas) -> Vec<String> {
    let dir = r.datos.join("anterior");
    if dir.join("servicio").exists() {
        return vec![];
    }
    let activo = |svc: &str| {
        s.ejecutar("systemctl", &["is-active", svc], None).is_ok_and(|o| o.trim() == "active")
    };
    let guardar = |nombre: &str, texto: &str| crate::sistema::escribir_privado(&dir.join(nombre), texto.as_bytes());
    let copiar = |sub: &str| {
        let origen = r.etc.join(sub);
        let destino = dir.join(format!("etc-{sub}"));
        let (o, d) = (origen.to_string_lossy().into_owned(), destino.to_string_lossy().into_owned());
        let _ = s.ejecutar("cp", &["-a", &o, &d], None);
    };
    let mut m = vec![];
    if activo("firewalld") {
        let zonas = s.ejecutar("firewall-cmd", &["--list-all-zones"], None).unwrap_or_default();
        let _ = guardar("firewalld.txt", &zonas);
        copiar("firewalld");
        let _ = guardar("servicio", "firewalld\n");
        match s.ejecutar("systemctl", &["disable", "--now", "firewalld"], None) {
            Ok(_) => m.push("firewalld desactivado; su configuración se ha guardado para poder volver a ella.".into()),
            Err(e) => m.push(format!("No se pudo desactivar firewalld: {e}")),
        }
    } else if s.ejecutar("ufw", &["status"], None).is_ok_and(|o| o.contains("Status: active")) {
        let estado = s.ejecutar("ufw", &["status", "verbose"], None).unwrap_or_default();
        let _ = guardar("ufw.txt", &estado);
        copiar("ufw");
        let _ = guardar("servicio", "ufw\n");
        match s.ejecutar("ufw", &["disable"], None) {
            Ok(_) => m.push("ufw desactivado; su configuración se ha guardado para poder volver a ella.".into()),
            Err(e) => m.push(format!("No se pudo desactivar ufw: {e}")),
        }
    }
    m
}

/// Al desinstalar con «restaurar»: quita la tabla y vuelve a activar lo que había.
pub fn restaurar_anterior(s: &dyn Sistema, r: &Rutas) -> Vec<String> {
    quitar_tabla(s);
    let ruta_servicio = r.datos.join("anterior/servicio");
    let servicio = std::fs::read_to_string(&ruta_servicio).unwrap_or_default();
    let r = match servicio.trim() {
        "firewalld" => s.ejecutar("systemctl", &["enable", "--now", "firewalld"], None).map(|_| "firewalld"),
        "ufw" => s.ejecutar("ufw", &["--force", "enable"], None).map(|_| "ufw"),
        _ => return vec!["No había otro firewall guardado: la máquina queda sin reglas de firewall.".into()],
    };
    match r {
        Ok(n) => {
            let _ = std::fs::remove_file(&ruta_servicio);
            vec![format!("{n} vuelve a estar activo con su configuración anterior.")]
        }
        Err(e) => vec![format!("No se pudo volver a activar el firewall anterior: {e}")],
    }
}

/// Quita la tabla `inet netctl`.
pub fn quitar_tabla(s: &dyn Sistema) {
    let _ = s.ejecutar("nft", &["delete", "table", "inet", "netctl"], None);
}

/// Desactiva los reenvíos caducados. Devuelve sus nombres.
pub fn revisar_caducados(c: &mut Config, ahora: u64) -> Vec<String> {
    let mut v = vec![];
    for r in c.reenvios.iter_mut().filter(|r| r.activo && r.caduca.is_some_and(|x| x <= ahora)) {
        r.activo = false;
        v.push(r.nombre.clone());
    }
    v
}

/// Actualiza la IP de los equipos vinculados por MAC. Devuelve un mensaje por cambio.
pub fn actualizar_ips(c: &mut Config, vecinos: &[Vecino]) -> Vec<String> {
    let mut mensajes = vec![];
    let mut tocar = |d: &mut Direccion| {
        // Solo se sigue al equipo si su MAC aparece conectada en una única IP de la misma
        // familia: con varias (alguien suplanta la MAC) o con entradas viejas no se toca nada.
        let candidatos = |mac: &str, ip: &std::net::IpAddr| -> Vec<&Vecino> {
            vecinos
                .iter()
                .filter(|v| v.conectado && v.mac.eq_ignore_ascii_case(mac) && v.ip.is_ipv4() == ip.is_ipv4())
                .collect()
        };
        if let Direccion::Equipo { mac, ip, nombre } = d
            && let [v] = candidatos(mac, ip).as_slice()
            && v.ip != *ip
        {
            mensajes.push(format!("La IP de «{nombre}» cambió de {ip} a {}: las reglas se han actualizado.", v.ip));
            *ip = v.ip;
        }
    };
    for r in &mut c.reglas {
        tocar(&mut r.origen);
        tocar(&mut r.destino);
    }
    for r in &mut c.reenvios {
        tocar(&mut r.equipo);
        r.desde.iter_mut().for_each(&mut tocar);
    }
    for g in &mut c.grupos_ip {
        g.miembros.iter_mut().for_each(&mut tocar);
    }
    mensajes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pruebas::*;
    use crate::sistema::Falso;

    fn montar(plazo_ms: u64) -> (tempfile::TempDir, Arc<Falso>, Aplicador) {
        let dir = tempfile::tempdir().unwrap();
        let s = Arc::new(Falso::new());
        let a = Aplicador::new(s.clone(), Rutas::en(dir.path()), Duration::from_millis(plazo_ms));
        (dir, s, a)
    }

    #[tokio::test]
    async fn sin_confirmar_revierte() {
        let (_d, s, a) = montar(50);
        let buena = router();
        a.aplicar(&buena, &fijas(), "admin", "inicial", false).unwrap();
        let mut mala = router();
        mala.ping_externo = false;
        s.limpiar();
        let e = a.aplicar(&mala, &fijas(), "admin", "quitar ping", true).unwrap().unwrap();
        assert!(a.en_espera().is_some());
        assert!(s.entradas("nft -f")[0].contains("iifname != { \"eth0\" } icmp type echo-request"));
        tokio::time::sleep(Duration::from_millis(300)).await;
        let entradas = s.entradas("nft -f");
        assert_eq!(entradas.len(), 2, "aplica la mala y después restaura la buena");
        assert!(entradas[1].contains("\t\ticmp type echo-request accept"));
        assert!(a.en_espera().is_none());
        assert!(!a.confirmar(e.id, "admin"));
        assert_eq!(a.almacen.cargar_confirmada().unwrap().unwrap(), buena);
        assert!(a.almacen.cargar_pendiente().unwrap().is_none());
        let reg = a.registro.listar(10);
        assert_eq!(reg[0].estado, EstadoCambio::Revertido);
    }

    #[tokio::test]
    async fn sin_confirmada_revierte_quitando_tabla() {
        let (_d, s, a) = montar(50);
        a.aplicar(&router(), &fijas(), "admin", "inicial", true).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(s.ordenes().contains(&"nft delete table inet netctl".to_string()));
    }

    #[tokio::test]
    async fn primer_cambio_sin_confirmar_devuelve_firewalld() {
        let (d, s, a) = montar(50);
        let r = Rutas::en(d.path());
        s.responder("systemctl is-active firewalld", Ok("active\n"));
        traspaso_inicial(&*s, &r);
        a.aplicar(&router(), &fijas(), "admin", "asistente", true).unwrap();
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(s.ordenes().contains(&"systemctl enable --now firewalld".to_string()), "{:?}", s.ordenes());
        assert!(!r.datos.join("anterior/servicio").exists(), "se puede volver a traspasar");
    }

    #[tokio::test]
    async fn confirmado_no_revierte() {
        let (_d, s, a) = montar(50);
        let c = router();
        let e = a.aplicar(&c, &fijas(), "admin", "inicial", true).unwrap().unwrap();
        assert!(a.confirmar(e.id, "admin"));
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(s.entradas("nft -f").len(), 1);
        assert_eq!(a.almacen.cargar_confirmada().unwrap().unwrap(), c);
        assert!(a.almacen.cargar_pendiente().unwrap().is_none());
        assert_eq!(a.registro.listar(1)[0].estado, EstadoCambio::Confirmado);
    }

    #[tokio::test]
    async fn segundo_cambio_revierte_a_la_confirmada() {
        let (_d, s, a) = montar(80);
        let buena = router();
        a.aplicar(&buena, &fijas(), "admin", "inicial", false).unwrap();
        let mut x = router();
        x.ping_externo = false;
        a.aplicar(&x, &fijas(), "admin", "uno", true).unwrap();
        let mut y = x.clone();
        y.nat.insert("interna".into(), false);
        a.aplicar(&y, &fijas(), "admin", "dos", true).unwrap();
        tokio::time::sleep(Duration::from_millis(400)).await;
        let e = s.entradas("nft -f");
        assert_eq!(e.len(), 4, "{e:?}");
        assert_eq!(e[3], nft::generar(&buena, &fijas(), 0).unwrap());
    }

    #[tokio::test]
    async fn arranque_usa_confirmada() {
        let (_d, s, a) = montar(60_000);
        let buena = router();
        a.almacen.guardar_confirmada(&buena).unwrap();
        let mut p = router();
        p.ping_externo = false;
        a.almacen.guardar_pendiente(&p).unwrap();
        let c = a.arrancar(|_| fijas()).unwrap().unwrap();
        assert_eq!(c, buena);
        assert!(a.almacen.cargar_pendiente().unwrap().is_none());
        assert!(s.entradas("nft -f")[0].contains("\t\ticmp type echo-request accept"));
    }

    #[tokio::test]
    async fn arranque_sin_confirmada_quita_lo_pendiente() {
        // Primer cambio (asistente) sin confirmar y el servicio se reinicia dentro de los 60 s.
        let (d, s, a) = montar(60_000);
        let r = Rutas::en(d.path());
        s.responder("systemctl is-active firewalld", Ok("active\n"));
        traspaso_inicial(&*s, &r);
        a.aplicar(&router(), &fijas(), "admin", "asistente", true).unwrap();
        s.limpiar();
        let a2 = Aplicador::new(s.clone(), r.clone(), Duration::from_secs(60));
        assert!(a2.arrancar(|_| fijas()).unwrap().is_none());
        let o = s.ordenes();
        assert!(o.contains(&"nft delete table inet netctl".to_string()), "{o:?}");
        assert!(o.contains(&"systemctl enable --now firewalld".to_string()), "{o:?}");
        assert!(a2.almacen.cargar_pendiente().unwrap().is_none());
    }

    #[test]
    fn mac_repetida_o_vieja_no_cambia_ip() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 80);
        r.equipo = Direccion::Equipo { mac: "aa:bb:cc:dd:ee:ff".into(), ip: "192.168.1.20".parse().unwrap(), nombre: "NAS".into() };
        c.reenvios.push(r);
        let v = |ip: &str, conectado| Vecino { ip: ip.parse().unwrap(), mac: "aa:bb:cc:dd:ee:ff".into(), iface: "eth1".into(), conectado };
        // dos IPs con la misma MAC (alguien la suplanta): no se toca
        assert!(actualizar_ips(&mut c, &[v("192.168.1.20", true), v("192.168.1.66", true)]).is_empty());
        // solo una entrada vieja (no conectada): no se toca
        assert!(actualizar_ips(&mut c, &[v("192.168.1.66", false)]).is_empty());
        assert!(nft::generar(&c, &fijas(), 0).unwrap().contains("dnat ip to 192.168.1.20"));
    }

    #[tokio::test]
    async fn nft_falla_no_cambia() {
        let (_d, s, a) = montar(60_000);
        let buena = router();
        a.aplicar(&buena, &fijas(), "admin", "inicial", false).unwrap();
        s.responder("nft -f", Err("Error: syntax error"));
        let mut otra = router();
        otra.ping_externo = false;
        let e = a.aplicar(&otra, &fijas(), "admin", "x", true).unwrap_err();
        assert!(e.contains("no se ha cambiado nada") && e.contains("syntax error"), "{e}");
        assert_eq!(a.almacen.cargar_confirmada().unwrap().unwrap(), buena);
        assert!(a.almacen.cargar_pendiente().unwrap().is_none());
        assert!(a.en_espera().is_none());
        assert_eq!(a.registro.listar(1)[0].estado, EstadoCambio::Error);
    }

    #[tokio::test]
    async fn config_invalida_no_toca_nada() {
        let (_d, s, a) = montar(60_000);
        let mut c = router();
        c.reenvios.push(reenvio(1, "192.168.1.20", 8443));
        assert!(a.aplicar(&c, &fijas(), "admin", "x", true).is_err());
        assert!(s.entradas("nft").is_empty());
    }

    #[test]
    fn traspaso_firewalld() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rutas::en(dir.path());
        let s = Falso::new();
        s.responder("systemctl is-active firewalld", Ok("active\n"));
        s.responder("systemctl is-active ufw", Err("inactive"));
        s.responder("firewall-cmd --list-all-zones", Ok("public (active)\n  services: ssh\n"));
        let m = traspaso_inicial(&s, &r);
        assert!(!m.is_empty());
        let o = s.ordenes();
        assert!(o.iter().any(|x| x.starts_with("cp -a") && x.contains("firewalld")), "{o:?}");
        assert!(o.contains(&"systemctl disable --now firewalld".to_string()), "{o:?}");
        let g = std::fs::read_to_string(r.datos.join("anterior/firewalld.txt")).unwrap();
        assert!(g.contains("services: ssh"));
        assert_eq!(std::fs::read_to_string(r.datos.join("anterior/servicio")).unwrap().trim(), "firewalld");
        // segunda vez no vuelve a guardar
        s.limpiar();
        traspaso_inicial(&s, &r);
        assert!(!s.ordenes().iter().any(|x| x.contains("disable")));
    }

    #[test]
    fn traspaso_ufw() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rutas::en(dir.path());
        let s = Falso::new();
        s.responder("systemctl is-active firewalld", Err("inactive"));
        s.responder("ufw status", Ok("Status: active\nTo Action From\n22 ALLOW Anywhere\n"));
        traspaso_inicial(&s, &r);
        assert!(s.ordenes().contains(&"ufw disable".to_string()));
        assert_eq!(std::fs::read_to_string(r.datos.join("anterior/servicio")).unwrap().trim(), "ufw");
    }

    #[test]
    fn nada_que_traspasar() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rutas::en(dir.path());
        let s = Falso::new();
        s.responder("systemctl is-active", Err("inactive"));
        s.responder("ufw", Err("no existe"));
        assert!(traspaso_inicial(&s, &r).is_empty());
        assert!(!r.datos.join("anterior/servicio").exists());
    }

    #[test]
    fn restaurar_firewalld() {
        let dir = tempfile::tempdir().unwrap();
        let r = Rutas::en(dir.path());
        let s = Falso::new();
        s.responder("systemctl is-active firewalld", Ok("active\n"));
        s.responder("systemctl is-active ufw", Err("inactive"));
        traspaso_inicial(&s, &r);
        s.limpiar();
        let m = restaurar_anterior(&s, &r);
        let o = s.ordenes();
        assert!(o.contains(&"nft delete table inet netctl".to_string()));
        assert!(o.contains(&"systemctl enable --now firewalld".to_string()), "{o:?}");
        assert!(m.iter().any(|x| x.contains("firewalld")));
    }

    #[test]
    fn caducidad_desactiva() {
        let mut c = router();
        let mut r = reenvio(1, "192.168.1.20", 80);
        r.caduca = Some(100);
        c.reenvios.push(r);
        c.reenvios.push(reenvio(2, "192.168.1.21", 81));
        assert!(revisar_caducados(&mut c, 99).is_empty());
        assert_eq!(revisar_caducados(&mut c, 100), vec!["r1".to_string()]);
        assert!(!c.reenvios[0].activo);
        assert!(c.reenvios[1].activo);
        assert_eq!(c.reenvios.len(), 2, "no se borra");
        assert!(revisar_caducados(&mut c, 200).is_empty(), "solo avisa una vez");
    }

    #[test]
    fn mac_cambia_ip_regenera() {
        let mut c = router();
        let nas = Direccion::Equipo { mac: "aa:bb:cc:dd:ee:ff".into(), ip: "192.168.1.20".parse().unwrap(), nombre: "NAS".into() };
        let mut r = reenvio(1, "192.168.1.20", 80);
        r.equipo = nas.clone();
        c.reenvios.push(r);
        let mut regla = regla(2, "interna", "externa", crate::modelo::Accion::Bloquear);
        regla.origen = nas;
        c.reglas.push(regla);
        let vecinos = vec![Vecino { ip: "192.168.1.77".parse().unwrap(), mac: "AA:BB:CC:DD:EE:FF".into(), iface: "eth1".into(), conectado: true }];
        let m = actualizar_ips(&mut c, &vecinos);
        assert_eq!(m.len(), 2, "{m:?}");
        assert!(m[0].contains("NAS") && m[0].contains("192.168.1.20") && m[0].contains("192.168.1.77"));
        let t = nft::generar(&c, &fijas(), 0).unwrap();
        assert!(t.contains("dnat ip to 192.168.1.77") && t.contains("ip saddr 192.168.1.77"));
        assert!(actualizar_ips(&mut c, &vecinos).is_empty());
        // un vecino IPv6 con la misma MAC no cambia una IPv4
        let v6 = vec![Vecino { ip: "fe80::1".parse().unwrap(), mac: "aa:bb:cc:dd:ee:ff".into(), iface: "eth1".into(), conectado: true }];
        assert!(actualizar_ips(&mut c, &v6).is_empty());
    }
}
