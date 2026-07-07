//! The standalone HTTPS + WSS bridge the Office task pane talks to.
//!
//! The task pane (served embedded from [`assets`]) runs inside Office's
//! WebView2 and must reach ziee over a *locally-trusted* `https://localhost`
//! origin with a same-origin `wss://` upgrade (WebView2 refuses mixed content
//! and un-trusted certs). This sub-module owns the pieces that make that work:
//!
//! - [`cert`] — mints (and caches) the self-signed `localhost` trust anchor
//!   whose SAN covers `localhost` + `127.0.0.1` + `::1` (DEC-5). ITEM-4.
//! - [`assets`] — the embedded add-in bundle (manifest + task pane + icon),
//!   baked into the binary via `include_dir!`. ITEM-12.
//!
//! The rustls-served axum listener + WSS `/bridge` upgrade + per-session token
//! auth land in ITEM-5 and consume both of the above.

pub mod assets;
pub mod cert;
