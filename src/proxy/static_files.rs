//! Static file serving for the embedded SPA
//!
//! Uses rust-embed to embed frontend/dist/ into the binary at compile time.
//! Serves index.html as fallback for SPA routing.

use axum::{
    http::{header, StatusCode, Uri},
    response::{Html, IntoResponse, Response},
};
use rust_embed::Embed;

#[derive(Embed)]
#[folder = "frontend/dist/"]
struct Assets;

pub async fn spa_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    // Try exact path first
    if let Some(file) = Assets::get(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, mime.as_ref())],
            file.data.to_vec(),
        )
            .into_response();
    }

    // Fallback to index.html for SPA routing
    match Assets::get("index.html") {
        Some(file) => Html(String::from_utf8_lossy(&file.data).into_owned()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}
