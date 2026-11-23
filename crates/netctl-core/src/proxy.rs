//! Proxy autenticado hacia el socket Unix de cada módulo, y ficheros `ui/` de los módulos.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderName, HeaderValue, StatusCode, header};
use axum::response::Response;
use axum::routing::{any, get};
use axum::{Extension, Router};
use hyper::body::Incoming;
use hyper_util::rt::TokioIo;

use crate::api::{CABECERA_CSRF, Compartido, error};
use crate::auth::USUARIO;
use crate::modulos::{Gestor, id_valido};

pub const CABECERA_USUARIO: &str = "x-netctl-usuario";
/// Tiempo máximo hasta las cabeceras de respuesta (el cuerpo puede ser un flujo SSE sin fin).
const ESPERA_CABECERAS: Duration = Duration::from_secs(60);

/// Envía `req` por HTTP/1.1 al socket Unix `socket`.
pub async fn enviar(socket: &Path, req: Request) -> anyhow::Result<hyper::Response<Incoming>> {
    let flujo = tokio::net::UnixStream::connect(socket).await?;
    let (mut tx, conexion) = hyper::client::conn::http1::handshake(TokioIo::new(flujo)).await?;
    tokio::spawn(async move {
        let _ = conexion.await;
    });
    Ok(tx.send_request(req).await?)
}

/// Cabeceras que no se reenvían en ninguna dirección.
fn salto_a_salto(n: &HeaderName) -> bool {
    matches!(
        n.as_str(),
        "connection"
            | "keep-alive"
            | "proxy-connection"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
    )
}

/// Parte la ruta cruda (sin decodificar) `/<a>/<b>/<id>/<resto>` en `(id, "/<resto>")`.
fn id_y_resto(ruta: &str, saltar: usize) -> Option<(&str, &str)> {
    let mut trozos = ruta.splitn(saltar + 3, '/');
    for _ in 0..=saltar {
        trozos.next()?;
    }
    let id = trozos.next()?;
    let resto = trozos.next()?;
    let inicio = ruta.len() - resto.len() - 1;
    id_valido(id).then_some((id, &ruta[inicio..]))
}

async fn reenviar(Extension(g): Extension<Arc<Gestor>>, req: Request) -> Response {
    // Se usa la ruta tal cual llegó (sin decodificar `%2F` ni similares).
    let Some((id, resto)) = id_y_resto(req.uri().path(), 2) else {
        return error(StatusCode::NOT_FOUND, "Ese módulo no existe");
    };
    let id = id.to_string();
    let destino = match req.uri().query() {
        Some(q) => format!("{resto}?{q}"),
        None => resto.to_string(),
    };
    let (partes, cuerpo) = req.into_parts();
    let mut nueva = axum::http::Request::builder()
        .method(partes.method)
        .uri(destino)
        .version(axum::http::Version::HTTP_11);
    for (n, v) in &partes.headers {
        let fuera = salto_a_salto(n)
            || n == header::HOST
            || n == header::COOKIE
            || n.as_str() == CABECERA_CSRF
            || n.as_str() == CABECERA_USUARIO;
        if !fuera {
            nueva = nueva.header(n, v);
        }
    }
    let nueva = nueva
        .header(header::HOST, "localhost")
        .header(CABECERA_USUARIO, USUARIO)
        .body(cuerpo);
    let Ok(nueva) = nueva else {
        return error(StatusCode::BAD_REQUEST, "Petición no válida");
    };
    let socket = g.rutas().socket(&id);
    let r = match tokio::time::timeout(ESPERA_CABECERAS, enviar(&socket, nueva)).await {
        Ok(Ok(r)) => r,
        Ok(Err(_)) => {
            return error(
                StatusCode::BAD_GATEWAY,
                format!("El módulo {id} no responde"),
            );
        }
        Err(_) => {
            return error(
                StatusCode::GATEWAY_TIMEOUT,
                format!("El módulo {id} tarda demasiado en responder"),
            );
        }
    };
    let (mut partes, cuerpo) = r.into_parts();
    // Nada de cookies desde los módulos: la única es la de sesión del núcleo.
    let quitar: Vec<HeaderName> = partes
        .headers
        .keys()
        .filter(|n| salto_a_salto(n) || *n == header::SET_COOKIE)
        .cloned()
        .collect();
    for n in quitar {
        partes.headers.remove(n);
    }
    Response::from_parts(partes, Body::new(cuerpo))
}

async fn ui_modulo(Extension(g): Extension<Arc<Gestor>>, req: Request) -> Response {
    let no = || error(StatusCode::NOT_FOUND, "No existe");
    // `/m/<id>/ui/<f>`
    let Some((id, resto)) = id_y_resto(req.uri().path(), 1) else {
        return no();
    };
    let Some(f) = resto.strip_prefix("/ui/") else {
        return no();
    };
    // Nombres sencillos y sin escapes: nada de `..`, `%`, `\\` ni rutas vacías.
    let valido = |t: &str| {
        !t.is_empty()
            && t != "."
            && t != ".."
            && t.chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
    };
    if !f.split('/').all(valido) {
        return no();
    }
    let ruta = g.rutas().dir_modulo(id).join("ui").join(f);
    match tokio::fs::symlink_metadata(&ruta).await {
        Ok(m) if m.is_file() => {}
        _ => return no(),
    }
    match tokio::fs::read(&ruta).await {
        Ok(datos) => {
            let mut r = Response::new(Body::from(datos));
            r.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static(crate::web::tipo_mime(f)),
            );
            r.headers_mut()
                .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-cache"));
            r
        }
        Err(_) => no(),
    }
}

/// `ANY /api/m/{id}/{*resto}` y `GET /m/{id}/ui/{*f}` (requieren sesión).
pub fn rutas(g: Arc<Gestor>) -> Router<Compartido> {
    Router::new()
        .route("/api/m/{id}/{*resto}", any(reenviar))
        .route("/m/{id}/ui/{*f}", get(ui_modulo))
        .layer(Extension(g))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::pruebas::*;
    use crate::modulos::pruebas::*;
    use http_body_util::BodyExt;
    use serde_json::json;
    use tower::ServiceExt;

    async fn preparar(d: &Path) -> (Router, Arc<Gestor>) {
        let (g, _) = gestor(d, "http://127.0.0.1:9/", None);
        let app = app_desde(estado(d), super::rutas(g.clone()), "192.168.1.20");
        (app, g)
    }

    #[tokio::test]
    async fn proxy_reenvia() {
        let d = tempfile::tempdir().unwrap();
        let (app, g) = preparar(d.path()).await;
        let _m = modulo_falso(&g.rutas().socket("prueba")).await;
        let s = entrar(&app).await;
        let (st, _, v) = pedir(
            &app,
            peticion("GET", "/api/m/prueba/eco/a/b?x=1&y=%20", Some(&s), None),
        )
        .await;
        assert_eq!(st, StatusCode::OK, "{v}");
        assert_eq!(v["ruta"], "/eco/a/b?x=1&y=%20");
        assert_eq!(v["usuario"], "admin");
        assert_eq!(v["cookie"], false, "la cookie de sesión no llega al módulo");
        assert_eq!(v["csrf"], false);
        let (st, _, v) = pedir(
            &app,
            peticion(
                "POST",
                "/api/m/prueba/eco/z",
                Some(&s),
                Some(json!({"a": 1})),
            ),
        )
        .await;
        assert_eq!(st, StatusCode::OK);
        assert_eq!(v["cuerpo"], r#"{"a":1}"#);
        // Una cabecera de usuario falsa del navegador se sustituye.
        let mut r = peticion("GET", "/api/m/prueba/eco/q", Some(&s), None);
        r.headers_mut()
            .insert(CABECERA_USUARIO, HeaderValue::from_static("root"));
        let (_, _, v) = pedir(&app, r).await;
        assert_eq!(v["usuario"], "admin");
    }

    #[tokio::test]
    async fn proxy_sin_sesion_401() {
        let d = tempfile::tempdir().unwrap();
        let (app, g) = preparar(d.path()).await;
        let _m = modulo_falso(&g.rutas().socket("prueba")).await;
        let (st, _, _) = pedir(&app, peticion("GET", "/api/m/prueba/eco/a", None, None)).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
        let (st, _, _) = pedir(&app, peticion("GET", "/m/prueba/ui/index.js", None, None)).await;
        assert_eq!(st, StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn proxy_modulo_caido_502() {
        let d = tempfile::tempdir().unwrap();
        let (app, _) = preparar(d.path()).await;
        let s = entrar(&app).await;
        let (st, _, v) = pedir(&app, peticion("GET", "/api/m/prueba/salud", Some(&s), None)).await;
        assert_eq!(st, StatusCode::BAD_GATEWAY);
        assert!(v["error"].is_string());
        let (st, _, _) = pedir(&app, peticion("GET", "/api/m/..%2Fx/salud", Some(&s), None)).await;
        assert_eq!(st, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn proxy_streaming() {
        let d = tempfile::tempdir().unwrap();
        let (app, g) = preparar(d.path()).await;
        let _m = modulo_falso(&g.rutas().socket("prueba")).await;
        let s = entrar(&app).await;
        let r = app
            .clone()
            .oneshot(peticion("GET", "/api/m/prueba/flujo", Some(&s), None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert_eq!(r.headers()[header::CONTENT_TYPE], "text/event-stream");
        let mut cuerpo = r.into_body();
        let trozo = tokio::time::timeout(Duration::from_secs(3), cuerpo.frame())
            .await
            .expect("sin streaming");
        let datos = trozo.unwrap().unwrap().into_data().unwrap();
        assert_eq!(&datos[..], b"data: uno\n\n");
    }

    #[tokio::test]
    async fn ui_del_modulo() {
        let d = tempfile::tempdir().unwrap();
        let (app, g) = preparar(d.path()).await;
        crate::modulos::instalar::extraer(
            &tar_modulo("prueba", "0.1.0", "0.1.0"),
            &g.rutas().dir_modulo("prueba"),
        )
        .unwrap();
        std::fs::write(d.path().join("secreto"), "x").unwrap();
        let s = entrar(&app).await;
        let r = app
            .clone()
            .oneshot(peticion("GET", "/m/prueba/ui/index.js", Some(&s), None))
            .await
            .unwrap();
        assert_eq!(r.status(), StatusCode::OK);
        assert!(
            r.headers()[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with("text/javascript")
        );
        for malo in [
            "/m/prueba/ui/../manifest.toml",
            "/m/prueba/ui/%2E%2E/manifest.toml",
            "/m/prueba/ui/..%2F..%2F..%2Fsecreto",
            "/m/prueba/ui/nada.js",
        ] {
            let r = app
                .clone()
                .oneshot(peticion("GET", malo, Some(&s), None))
                .await
                .unwrap();
            assert_eq!(r.status(), StatusCode::NOT_FOUND, "{malo}");
        }
    }
}
