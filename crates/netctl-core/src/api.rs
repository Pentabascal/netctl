//! Rutas HTTP del núcleo: login, ajustes, filtro de origen, sesión y CSRF.

use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;

use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{any, get, post};
use axum::{Extension, Json, Router};
use ipnet::IpNet;
use netctl_sdk::confirmar::{IdCambio, Pendiente};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::watch;

use crate::alcance;
use crate::auth::{Almacen, ErrorLogin, Sesion, USUARIO};
use crate::config::{Alcance, Config};

pub const COOKIE: &str = "netctl_sesion";
pub const CABECERA_CSRF: &str = "x-netctl-csrf";
pub const PLAZO_CONFIRMACION: Duration = Duration::from_secs(60);

type RedesWg = Arc<dyn Fn() -> Vec<IpNet> + Send + Sync>;

pub struct Estado {
    pub config: RwLock<Config>,
    pub ruta_config: PathBuf,
    pub auth: Almacen,
    pub pendiente: Pendiente,
    /// Cambio de alcance a la espera de confirmación.
    pub cambio_acceso: Mutex<Option<IdCambio>>,
    pub plazo_confirmacion: Duration,
    /// Redes de las interfaces WireGuard (para el filtro `SoloLan`).
    pub redes_wg: RedesWg,
    /// Avisa al servidor para que vuelva a enlazar los puertos al cambiar el alcance.
    pub alcance_tx: watch::Sender<Alcance>,
}

impl Estado {
    pub fn new(config: Config, ruta_config: PathBuf, auth: Almacen, redes_wg: RedesWg) -> Self {
        let (alcance_tx, _) = watch::channel(config.alcance);
        Self {
            config: RwLock::new(config),
            ruta_config,
            auth,
            pendiente: Pendiente::new(),
            cambio_acceso: Mutex::new(None),
            plazo_confirmacion: PLAZO_CONFIRMACION,
            redes_wg,
            alcance_tx,
        }
    }

    pub fn alcance(&self) -> Alcance {
        self.config.read().unwrap().alcance
    }

    /// Guarda el alcance en `core.toml` y avisa al servidor.
    fn fijar_alcance(&self, a: Alcance) -> anyhow::Result<()> {
        let mut c = self.config.write().unwrap();
        let nueva = Config {
            alcance: a,
            ..c.clone()
        };
        nueva.guardar(&self.ruta_config)?;
        *c = nueva;
        self.alcance_tx.send_replace(a);
        Ok(())
    }
}

pub type Compartido = Arc<Estado>;

pub fn error(codigo: StatusCode, mensaje: impl Into<String>) -> Response {
    (codigo, Json(json!({ "error": mensaje.into() }))).into_response()
}

/// Router completo. `protegidas` son rutas extra (módulos, proxy) que exigen sesión.
pub fn router(estado: Compartido, protegidas: Router<Compartido>) -> Router {
    let privadas = Router::new()
        .route("/api/logout", post(logout))
        .route("/api/yo", get(yo))
        .route("/api/ajustes/acceso", get(ver_acceso).put(cambiar_acceso))
        .route("/api/ajustes/acceso/confirmar", post(confirmar_acceso))
        .route("/api/ajustes/totp", post(activar_totp))
        .route("/api/ajustes/totp/confirmar", post(confirmar_totp))
        .route(
            "/api/ajustes/password",
            axum::routing::put(cambiar_password),
        )
        .merge(protegidas)
        .route_layer(middleware::from_fn_with_state(
            estado.clone(),
            exigir_sesion,
        ));
    Router::new()
        .route("/api/login", post(login))
        .merge(privadas)
        .route(
            "/api/{*resto}",
            any(|| async { error(StatusCode::NOT_FOUND, "No existe") }),
        )
        .fallback(crate::web::servir)
        .layer(middleware::from_fn_with_state(
            estado.clone(),
            filtro_origen,
        ))
        .layer(middleware::from_fn(cabeceras_seguridad))
        .with_state(estado)
}

fn ip_origen(req: &Request) -> Option<IpAddr> {
    let ext = req.extensions();
    let addr = ext.get::<ConnectInfo<SocketAddr>>().map(|c| c.0);
    let addr = addr.or_else(|| {
        ext.get::<axum::extract::connect_info::MockConnectInfo<SocketAddr>>()
            .map(|c| c.0)
    });
    addr.map(|a| a.ip().to_canonical())
}

/// Rechaza conexiones de orígenes no permitidos por el alcance.
async fn filtro_origen(State(e): State<Compartido>, req: Request, next: Next) -> Response {
    let ok = ip_origen(&req).is_some_and(|ip| alcance::permitida(e.alcance(), ip, &(e.redes_wg)()));
    if !ok {
        return error(StatusCode::FORBIDDEN, "Acceso no permitido desde esta red");
    }
    next.run(req).await
}

async fn cabeceras_seguridad(req: Request, next: Next) -> Response {
    let mut r = next.run(req).await;
    let h = r.headers_mut();
    h.insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    h.insert("x-frame-options", HeaderValue::from_static("DENY"));
    h.insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    h.insert(
        "strict-transport-security",
        HeaderValue::from_static("max-age=31536000"),
    );
    r
}

/// Valor de la cookie de sesión.
pub fn token_cookie(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|p| p.trim().split_once('='))
        .find(|(k, _)| *k == COOKIE)
        .map(|(_, v)| v.to_string())
}

/// Exige sesión (401) y, en métodos que cambian algo, la cabecera CSRF (403).
async fn exigir_sesion(State(e): State<Compartido>, mut req: Request, next: Next) -> Response {
    let Some(sesion) = token_cookie(req.headers()).and_then(|t| e.auth.sesion(&t)) else {
        return error(StatusCode::UNAUTHORIZED, "Inicia sesión");
    };
    if !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        let csrf = req
            .headers()
            .get(CABECERA_CSRF)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !sesion.csrf_valido(csrf) {
            return error(
                StatusCode::FORBIDDEN,
                "Falta la cabecera CSRF o no es válida",
            );
        }
    }
    req.extensions_mut().insert(sesion);
    next.run(req).await
}

fn cookie_sesion(valor: &str, max_age: Option<u64>) -> HeaderValue {
    let edad = max_age
        .map(|s| format!("; Max-Age={s}"))
        .unwrap_or_default();
    HeaderValue::from_str(&format!(
        "{COOKIE}={valor}; HttpOnly; Secure; SameSite=Strict; Path=/{edad}"
    ))
    .unwrap()
}

#[derive(Deserialize)]
struct PeticionLogin {
    password: String,
    #[serde(default)]
    totp: Option<String>,
}

async fn login(
    State(e): State<Compartido>,
    ConnectInfo(origen): ConnectInfo<SocketAddr>,
    Json(p): Json<PeticionLogin>,
) -> Response {
    let ip = origen.ip().to_canonical();
    let e2 = e.clone();
    let r = tokio::task::spawn_blocking(move || {
        let totp = p.totp.as_deref().map(str::trim).filter(|t| !t.is_empty());
        e2.auth.login(&p.password, totp, ip)
    })
    .await;
    match r {
        Ok(Ok(s)) => {
            let mut r = Json(json!({ "usuario": USUARIO, "csrf": s.csrf })).into_response();
            r.headers_mut()
                .insert(header::SET_COOKIE, cookie_sesion(&s.token, None));
            r
        }
        Ok(Err(err @ ErrorLogin::TotpRequerido)) => (
            StatusCode::UNAUTHORIZED,
            Json(json!({ "error": err.mensaje(), "totp_requerido": true })),
        )
            .into_response(),
        Ok(Err(err @ ErrorLogin::Bloqueado { segundos })) => {
            let mut r = (
                StatusCode::TOO_MANY_REQUESTS,
                Json(json!({ "error": err.mensaje(), "segundos": segundos })),
            )
                .into_response();
            r.headers_mut()
                .insert(header::RETRY_AFTER, HeaderValue::from(segundos));
            r
        }
        Ok(Err(err)) => error(StatusCode::UNAUTHORIZED, err.mensaje()),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "Error interno"),
    }
}

async fn logout(State(e): State<Compartido>, Extension(s): Extension<Sesion>) -> Response {
    e.auth.cerrar(&s.token);
    let mut r = StatusCode::NO_CONTENT.into_response();
    r.headers_mut()
        .insert(header::SET_COOKIE, cookie_sesion("", Some(0)));
    r
}

async fn yo(State(e): State<Compartido>, Extension(s): Extension<Sesion>) -> Response {
    Json(json!({
        "usuario": USUARIO,
        "csrf": s.csrf,
        "totp_activo": e.auth.totp_activo(),
        "password_defecto": e.auth.usa_password_defecto(),
        "alcance": e.alcance(),
        "version": env!("CARGO_PKG_VERSION"),
    }))
    .into_response()
}

fn json_acceso(e: &Estado) -> Response {
    let pendiente = *e.cambio_acceso.lock().unwrap();
    Json(json!({ "alcance": e.alcance(), "totp_activo": e.auth.totp_activo(), "pendiente": pendiente }))
        .into_response()
}

async fn ver_acceso(State(e): State<Compartido>) -> Response {
    json_acceso(&e)
}

#[derive(Deserialize)]
struct PeticionAcceso {
    alcance: Alcance,
}

/// Cambia el alcance; se deshace solo si no se confirma en `plazo_confirmacion`.
async fn cambiar_acceso(State(e): State<Compartido>, Json(p): Json<PeticionAcceso>) -> Response {
    if p.alcance == Alcance::Internet && !e.auth.totp_activo() {
        return error(StatusCode::CONFLICT, "Activa primero el doble factor");
    }
    if p.alcance == Alcance::Internet && e.auth.usa_password_defecto() {
        return error(StatusCode::CONFLICT, "Cambia primero la contraseña de fábrica");
    }
    let anterior = e.alcance();
    if p.alcance == anterior {
        return json_acceso(&e);
    }
    {
        // Se reserva el hueco (id 0) y se suelta el cerrojo: `restaurar` también lo toma.
        let mut cambio = e.cambio_acceso.lock().unwrap();
        if cambio.is_some() {
            return error(
                StatusCode::CONFLICT,
                "Hay un cambio de acceso pendiente de confirmar",
            );
        }
        *cambio = Some(0);
    }
    let e2 = e.clone();
    let restaurar = move || {
        *e2.cambio_acceso.lock().unwrap() = None;
        match e2.fijar_alcance(anterior) {
            Ok(()) => eprintln!("Cambio de alcance no confirmado: se vuelve al anterior"),
            Err(err) => eprintln!("No se pudo restaurar el alcance anterior: {err:#}"),
        }
    };
    match e.pendiente.aplicar(
        || e.fijar_alcance(p.alcance),
        restaurar,
        e.plazo_confirmacion,
    ) {
        Ok(id) => {
            let mut cambio = e.cambio_acceso.lock().unwrap();
            if *cambio == Some(0) {
                *cambio = Some(id);
            }
            drop(cambio);
            Json(json!({ "id": id, "segundos": e.plazo_confirmacion.as_secs(), "alcance": p.alcance }))
                .into_response()
        }
        Err(err) => {
            *e.cambio_acceso.lock().unwrap() = None;
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("No se pudo cambiar el acceso: {err:#}"),
            )
        }
    }
}

#[derive(Deserialize)]
struct PeticionConfirmar {
    id: IdCambio,
}

async fn confirmar_acceso(
    State(e): State<Compartido>,
    Json(p): Json<PeticionConfirmar>,
) -> Response {
    let mut cambio = e.cambio_acceso.lock().unwrap();
    if *cambio == Some(p.id) && e.pendiente.confirmar(p.id) {
        *cambio = None;
        drop(cambio);
        return json_acceso(&e);
    }
    error(
        StatusCode::NOT_FOUND,
        "No hay ningún cambio pendiente con ese número (puede que ya se haya deshecho)",
    )
}

async fn activar_totp(State(e): State<Compartido>) -> Response {
    match e.auth.activar_totp() {
        Ok((secreto, uri)) => Json(json!({ "secreto": secreto, "uri": uri })).into_response(),
        Err(err) => error(StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")),
    }
}

#[derive(Deserialize)]
struct PeticionCodigo {
    codigo: String,
}

async fn confirmar_totp(State(e): State<Compartido>, Json(p): Json<PeticionCodigo>) -> Response {
    if e.auth.confirmar_totp(p.codigo.trim()) {
        Json(json!({ "totp_activo": true })).into_response()
    } else {
        error(StatusCode::UNPROCESSABLE_ENTITY, "El código no es correcto")
    }
}

#[derive(Deserialize)]
struct PeticionPassword {
    actual: String,
    nueva: String,
}

/// Cambia la contraseña. Cierra todas las sesiones (también la actual).
async fn cambiar_password(
    State(e): State<Compartido>,
    Json(p): Json<PeticionPassword>,
) -> Response {
    if p.nueva.chars().count() < 10 {
        return error(
            StatusCode::UNPROCESSABLE_ENTITY,
            "La contraseña nueva debe tener al menos 10 caracteres",
        );
    }
    let r = tokio::task::spawn_blocking(move || {
        if !e.auth.comprobar_password(&p.actual) {
            return Err((
                StatusCode::FORBIDDEN,
                "La contraseña actual no es correcta".to_string(),
            ));
        }
        e.auth
            .fijar_password(&p.nueva)
            .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, format!("{err:#}")))
    })
    .await;
    match r {
        Ok(Ok(())) => {
            let mut r = StatusCode::NO_CONTENT.into_response();
            r.headers_mut()
                .insert(header::SET_COOKIE, cookie_sesion("", Some(0)));
            r
        }
        Ok(Err((codigo, mensaje))) => error(codigo, mensaje),
        Err(_) => error(StatusCode::INTERNAL_SERVER_ERROR, "Error interno"),
    }
}

#[cfg(test)]
pub(crate) mod pruebas {
    //! Utilidades de test compartidas con otros módulos.
    use super::*;
    use axum::body::Body;
    use axum::extract::connect_info::MockConnectInfo;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    pub const PASSWORD: &str = "contraseña-buena";

    pub fn estado(dir: &std::path::Path) -> Compartido {
        let auth = Almacen::abrir(&dir.join("admin.json")).unwrap();
        auth.fijar_password(PASSWORD).unwrap();
        let wg: Vec<IpNet> = vec!["10.8.0.0/24".parse().unwrap()];
        Arc::new(Estado::new(
            Config::default(),
            dir.join("core.toml"),
            auth,
            Arc::new(move || wg.clone()),
        ))
    }

    pub fn app_desde(estado: Compartido, protegidas: Router<Compartido>, origen: &str) -> Router {
        let addr: SocketAddr = format!("{origen}:50000").parse().unwrap();
        router(estado, protegidas).layer(MockConnectInfo(addr))
    }

    pub async fn pedir(
        app: &Router,
        req: axum::http::Request<Body>,
    ) -> (StatusCode, HeaderMap, serde_json::Value) {
        let r = app.clone().oneshot(req).await.unwrap();
        let (partes, cuerpo) = r.into_parts();
        let bytes = cuerpo.collect().await.unwrap().to_bytes();
        let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::Value::Null);
        (partes.status, partes.headers, v)
    }

    pub fn peticion(
        metodo: &str,
        ruta: &str,
        sesion: Option<&(String, String)>,
        cuerpo: Option<serde_json::Value>,
    ) -> axum::http::Request<Body> {
        let mut b = axum::http::Request::builder().method(metodo).uri(ruta);
        if let Some((cookie, csrf)) = sesion {
            b = b
                .header(header::COOKIE, format!("{COOKIE}={cookie}"))
                .header(CABECERA_CSRF, csrf);
        }
        match cuerpo {
            Some(c) => b
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(c.to_string()))
                .unwrap(),
            None => b.body(Body::empty()).unwrap(),
        }
    }

    /// Inicia sesión y devuelve `(token, csrf)`.
    pub async fn entrar(app: &Router) -> (String, String) {
        let (st, h, v) = pedir(
            app,
            peticion(
                "POST",
                "/api/login",
                None,
                Some(json!({ "password": PASSWORD })),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{v}");
        let cookie = h.get(header::SET_COOKIE).unwrap().to_str().unwrap();
        let token = cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_string();
        (token, v["csrf"].as_str().unwrap().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::pruebas::*;
    use super::*;

    fn app(dir: &std::path::Path) -> (Compartido, Router) {
        let e = estado(dir);
        (e.clone(), app_desde(e, Router::new(), "192.168.1.20"))
    }

    #[tokio::test]
    async fn sesion_cookie_flags() {
        let d = tempfile::tempdir().unwrap();
        let (_, app) = app(d.path());
        let (st, h, _) = pedir(
            &app,
            peticion(
                "POST",
                "/api/login",
                None,
                Some(json!({ "password": PASSWORD })),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        let c = h.get(header::SET_COOKIE).unwrap().to_str().unwrap();
        assert!(c.starts_with("netctl_sesion="));
        for f in ["HttpOnly", "Secure", "SameSite=Strict", "Path=/"] {
            assert!(c.contains(f), "falta {f} en {c}");
        }
        let (st, _, v) = pedir(
            &app,
            peticion(
                "POST",
                "/api/login",
                None,
                Some(json!({ "password": "mala" })),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        assert!(v["error"].is_string());
    }

    #[tokio::test]
    async fn csrf() {
        let d = tempfile::tempdir().unwrap();
        let (_, app) = app(d.path());
        let (token, csrf) = entrar(&app).await;
        let sin = (token.clone(), String::new());
        let (st, _, _) = pedir(&app, peticion("POST", "/api/logout", Some(&sin), None)).await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        let malo = (token.clone(), "otro".to_string());
        let (st, _, _) = pedir(&app, peticion("POST", "/api/logout", Some(&malo), None)).await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        // GET no necesita CSRF.
        let (st, _, v) = pedir(&app, peticion("GET", "/api/yo", Some(&sin), None)).await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["csrf"], csrf.as_str());
        let bien = (token, csrf);
        let (st, _, _) = pedir(&app, peticion("POST", "/api/logout", Some(&bien), None)).await;
        assert_eq!(st, StatusCode::NO_CONTENT);
        let (st, _, _) = pedir(&app, peticion("GET", "/api/yo", Some(&bien), None)).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn sin_sesion_401() {
        let d = tempfile::tempdir().unwrap();
        let (_, app) = app(d.path());
        let (st, _, _) = pedir(&app, peticion("GET", "/api/yo", None, None)).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        let (st, _, _) = pedir(
            &app,
            peticion(
                "PUT",
                "/api/ajustes/acceso",
                None,
                Some(json!({"alcance":"internet"})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn origen_publico_rechazado() {
        let d = tempfile::tempdir().unwrap();
        let e = estado(d.path());
        let publico = app_desde(e.clone(), Router::new(), "8.8.8.8");
        let (st, _, _) = pedir(
            &publico,
            peticion(
                "POST",
                "/api/login",
                None,
                Some(json!({ "password": PASSWORD })),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::FORBIDDEN);
        let vpn = app_desde(e, Router::new(), "10.8.0.7");
        entrar(&vpn).await;
    }

    #[tokio::test]
    async fn internet_exige_totp() {
        let d = tempfile::tempdir().unwrap();
        let (e, app) = app(d.path());
        let s = entrar(&app).await;
        let (st, _, v) = pedir(
            &app,
            peticion(
                "PUT",
                "/api/ajustes/acceso",
                Some(&s),
                Some(json!({"alcance":"internet"})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::CONFLICT);
        assert_eq!(v["error"], "Activa primero el doble factor");
        assert_eq!(e.alcance(), Alcance::SoloLan);
    }

    fn activar_totp_directo(e: &Estado) {
        let (secreto, _) = e.auth.activar_totp().unwrap();
        let s = totp_rs::Secret::Encoded(secreto).to_bytes().unwrap();
        let t = totp_rs::TOTP::new(totp_rs::Algorithm::SHA1, 6, 1, 30, s).unwrap();
        assert!(e.auth.confirmar_totp(&t.generate_current().unwrap()));
    }

    #[tokio::test]
    async fn cambio_de_alcance_sin_confirmar_se_deshace() {
        let d = tempfile::tempdir().unwrap();
        let mut e = estado(d.path());
        Arc::get_mut(&mut e).unwrap().plazo_confirmacion = Duration::from_millis(50);
        let app = app_desde(e.clone(), Router::new(), "192.168.1.20");
        let s = entrar(&app).await;
        activar_totp_directo(&e);
        let rx = e.alcance_tx.subscribe();
        let (st, _, v) = pedir(
            &app,
            peticion(
                "PUT",
                "/api/ajustes/acceso",
                Some(&s),
                Some(json!({"alcance":"internet"})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{v}");
        assert_eq!(e.alcance(), Alcance::Internet);
        assert!(rx.has_changed().unwrap());
        assert_eq!(
            Config::cargar(&d.path().join("core.toml")).unwrap().alcance,
            Alcance::Internet
        );
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(e.alcance(), Alcance::SoloLan);
        assert_eq!(
            Config::cargar(&d.path().join("core.toml")).unwrap().alcance,
            Alcance::SoloLan
        );
        let (st, _, _) = pedir(
            &app,
            peticion(
                "POST",
                "/api/ajustes/acceso/confirmar",
                Some(&s),
                Some(json!({"id": v["id"]})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn cambio_de_alcance_confirmado_se_queda() {
        let d = tempfile::tempdir().unwrap();
        let mut e = estado(d.path());
        Arc::get_mut(&mut e).unwrap().plazo_confirmacion = Duration::from_millis(100);
        let app = app_desde(e.clone(), Router::new(), "192.168.1.20");
        let s = entrar(&app).await;
        activar_totp_directo(&e);
        let (_, _, v) = pedir(
            &app,
            peticion(
                "PUT",
                "/api/ajustes/acceso",
                Some(&s),
                Some(json!({"alcance":"internet"})),
            ),
        )
        .await;
        let (st, _, _) = pedir(
            &app,
            peticion(
                "POST",
                "/api/ajustes/acceso/confirmar",
                Some(&s),
                Some(json!({"id": v["id"]})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert_eq!(e.alcance(), Alcance::Internet);
    }

    #[tokio::test]
    async fn spa_y_api_desconocida() {
        let d = tempfile::tempdir().unwrap();
        let (_, app) = app(d.path());
        let r = tower::ServiceExt::oneshot(app.clone(), peticion("GET", "/modulos", None, None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(
            r.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/html")
        );
        assert_eq!(r.headers()["x-frame-options"], "DENY");
        let (st, _, _) = pedir(&app, peticion("GET", "/api/nada", None, None)).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }
}
