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
//! auth are ITEM-5 and consume both of the above:
//!
//! - [`auth`] — per-session token mint / register / constant-time verify / revoke
//!   (DEC-6), mirroring `llm_local_runtime::proxy` in spirit.
//! - [`protocol`] — the JSON-RPC 2.0 envelope types (`BridgeRequest` /
//!   `BridgeResponse` / `BridgeEvent`); method dispatch lands in ITEM-9.
//! - [`server`] — the dual-stack rustls listener + WSS `/bridge` echo + token +
//!   Origin guards + POST sinks ([`server::start`] → [`server::BridgeHandle`]).

pub mod assets;
pub mod auth;
pub mod cert;
pub mod protocol;
pub mod server;
