//! Static file serving for production mode
//!
//! Embeds the UI files into the binary using rust-embed
//! and serves them via an axum handler.

use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "../ui/dist/"] // Path relative to Cargo.toml
pub struct Assets;

// Wrapper to access the RustEmbed trait method
impl Assets {
    fn get_file(path: &str) -> Option<rust_embed::EmbeddedFile> {
        <Self as RustEmbed>::get(path)
    }
}

/// Serve embedded static files with SPA fallback
///
/// - Serves exact file matches from embedded assets
/// - Falls back to index.html for unknown routes (SPA behavior)
pub async fn serve_embedded_files(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first
    if let Some(content) = Assets::get_file(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }

    // SPA fallback: serve index.html for unknown routes
    if let Some(content) = Assets::get_file("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
