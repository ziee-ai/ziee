//! Static file serving for production mode
//!
//! Embeds the **desktop** UI bundle into the binary using rust-embed
//! and serves it for any non-API request hitting the backend's HTTP
//! port. Reached by:
//!   - The Tauri webview, for any request that doesn't go through
//!     the `tauri://localhost/` protocol (rare; the bundle normally
//!     loads via `frontendDist`).
//!   - **Phones / browsers via the Remote Access ngrok tunnel.**
//!     Both surfaces get the SAME single bundle — the desktop UI
//!     workspace is the single source of UI truth for this binary.
//!
//! There used to be a separate web bundle (`src-app/ui/dist/`)
//! embedded here for the tunnel surface; that split has been removed
//! and all phone-facing UI (magic-link page, password fallback,
//! username hiding) lives in the desktop UI workspace gated by an
//! `isTauriView` runtime check.

use axum::{
    body::Body,
    http::{header, Response, StatusCode, Uri},
    response::IntoResponse,
};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
// src-app/desktop/ui/dist, relative to this crate's Cargo.toml
// (src-app/desktop/tauri). It must match tauri.conf.json's `frontendDist`
// ("../ui/dist"). Was "../../ui/dist/", which resolves to the REMOVED
// src-app/ui/dist bundle (see module doc) — a path that doesn't exist, so
// release builds (rust-embed embeds at compile time; debug reads from disk)
// failed with "folder '…/ui/dist/' does not exist".
#[folder = "../ui/dist/"]
pub struct Assets;

// Wrapper to access the RustEmbed trait method
impl Assets {
    fn get_file(path: &str) -> Option<rust_embed::EmbeddedFile> {
        <Self as RustEmbed>::get(path)
    }
}

// Cache-Control for content-hashed build assets: everything Vite emits under
// `assets/` carries a content hash in its filename (`index-WiM5Va7E.js`), so the
// URL changes whenever the bytes change — safe to cache forever + `immutable`
// (the browser won't even revalidate). A new deploy ships new filenames.
const IMMUTABLE_CACHE: &str = "public, max-age=31536000, immutable";
// `index.html` (and the SPA fallback) is NOT hashed and points at the current
// asset filenames, so it must always be revalidated or a client would keep
// booting an old bundle after a deploy.
const HTML_CACHE: &str = "no-cache";

/// Serve embedded static files with SPA fallback.
///
/// - Serves exact file matches from embedded assets, with a long-lived
///   `immutable` `Cache-Control` for content-hashed `assets/*` files and
///   `no-cache` for the (unhashed) HTML entrypoint.
/// - Falls back to `index.html` for unknown routes (SPA behavior).
///
/// Compression (br/gzip) is applied by the `CompressionLayer` wrapping this
/// handler at the router seam (see `backend/mod.rs`), so responses are
/// negotiated per the request's `Accept-Encoding` without this handler having
/// to pre-compress. This path serves the Remote Access tunnel (the local Tauri
/// webview loads assets over the `tauri://` protocol), where both caching and
/// compression matter most.
pub async fn serve_embedded_files(uri: Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/');

    // Try the exact path first
    if let Some(content) = Assets::get_file(path) {
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        // Hashed build assets live under `assets/`; the HTML entry does not.
        let cache_control = if path.starts_with("assets/") {
            IMMUTABLE_CACHE
        } else {
            HTML_CACHE
        };
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, mime.as_ref())
            .header(header::CACHE_CONTROL, cache_control)
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }

    // SPA fallback: serve index.html for unknown routes
    if let Some(content) = Assets::get_file("index.html") {
        return Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "text/html")
            .header(header::CACHE_CONTROL, HTML_CACHE)
            .body(Body::from(content.data.into_owned()))
            .unwrap();
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("Not Found"))
        .unwrap()
}
