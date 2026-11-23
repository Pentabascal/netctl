//! Carcasa web incrustada (`web/shell/dist`, Tarea 5). Mientras no exista se sirve una página
//! provisional. Las rutas que no son ficheros devuelven `index.html` (SPA).

use axum::body::Body;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};

#[derive(rust_embed::RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../web/shell/dist"]
#[allow_missing = true]
struct Carcasa;

const PROVISIONAL: &str = "<!doctype html><html lang=\"es\"><head><meta charset=\"utf-8\">\
<title>netctl</title></head><body style=\"background:#12151a;color:#e6e8eb;font-family:sans-serif\">\
<h1>netctl</h1><p>La interfaz web aún no está compilada (<code>web/shell/dist</code>).</p></body></html>";

fn tipo(ruta: &str) -> &'static str {
    match ruta.rsplit('.').next().unwrap_or("") {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "ico" => "image/x-icon",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "map" => "application/json",
        _ => "application/octet-stream",
    }
}

/// Tipo MIME por extensión (también para los ficheros `ui/` de los módulos).
pub fn tipo_mime(ruta: &str) -> &'static str {
    tipo(ruta)
}

pub async fn servir(uri: Uri) -> Response {
    let ruta = uri.path().trim_start_matches('/');
    if let Some(f) = (!ruta.is_empty()).then(|| Carcasa::get(ruta)).flatten() {
        return (
            [(header::CONTENT_TYPE, tipo(ruta))],
            Body::from(f.data.into_owned()),
        )
            .into_response();
    }
    match Carcasa::get("index.html") {
        Some(f) => (
            [(header::CONTENT_TYPE, tipo("index.html"))],
            Body::from(f.data.into_owned()),
        )
            .into_response(),
        None => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, tipo("index.html"))],
            PROVISIONAL,
        )
            .into_response(),
    }
}
