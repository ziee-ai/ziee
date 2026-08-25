//! Route registration for the code_sandbox HTTP surface.
//!
//! Two route families:
//!
//! 1. **Untyped legacy routes** (plain `axum::routing::{get, post}`):
//!    - POST `/code-sandbox`               — JSON-RPC MCP loopback
//!    - GET  `/code-sandbox/file/download` — workspace artifact download
//!    These are invoked by our own clients (MCP loopback, in-browser
//!    artifact link) and aren't typed via OpenAPI.
//!
//! 2. **Typed REST routes** (`aide::axum::routing::{get_with, post_with}`):
//!    Resource-limits singleton + rootfs-versions admin surface.
//!    The legacy flavor-keyed environments/prefetch endpoints retired
//!    with Plan 5 Phase 2c (SSE port to the version-aware install
//!    endpoint).
//!
//! `ApiRouter` accepts both `.route()` (untyped) and `.api_route()`
//! (typed) in the same router — they coexist cleanly.

use aide::axum::routing::{delete_with, get_with, post_with};
use aide::axum::ApiRouter;
use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};

use crate::modules::code_sandbox::{handlers, version_handlers};

pub fn code_sandbox_router() -> ApiRouter {
    ApiRouter::new()
        // ──────── Untyped legacy ────────
        // Note (Plan-3 Phase-3 / I2): the route is registered POST-only on
        // purpose. The MCP spec § Transports lets a server offer an
        // additional standalone GET-SSE for unsolicited server→client
        // messages (progress notifications, server-initiated sampling).
        // Our built-in code_sandbox server has no out-of-band producers —
        // every elicitation / progress / streamed output rides the POST
        // response — so we don't run a GET stream. axum's `MethodRouter`
        // turns a GET against this path into `405 Method Not Allowed`,
        // which is exactly the "no standalone stream" signal the spec
        // requires. The client (`mcp/client/http.rs::spawn_standalone_get_sse`)
        // tolerates 405 silently.
        // Per-route body limit raised above the global A3 16 MiB cap.
        // The sandbox's `write_file` tool genuinely accepts file payloads
        // up to its internal 32 MiB content cap (enforced in the tool
        // handler — see tier6_security_regression::e2e_write_file_rejects_
        // oversized_content). Without this override, A3's global limit
        // would 413 the request before the sandbox's own validation
        // could return a structured JSON-RPC error envelope.
        // Set to 64 MiB so legitimate write_file calls succeed and
        // oversized calls reach the sandbox's 32 MiB cap (which returns
        // the proper error shape).
        .route(
            "/code-sandbox",
            post(handlers::jsonrpc_handler).layer(DefaultBodyLimit::max(64 * 1024 * 1024)),
        )
        .route(
            "/code-sandbox/file/download",
            get(handlers::download_handler),
        )
        // ──────── Resource limits (Plan 1 §6) ────────
        .api_route(
            "/code-sandbox/resource-limits",
            get_with(
                handlers::get_resource_limits_handler,
                handlers::get_resource_limits_docs,
            )
            .put_with(
                handlers::update_resource_limits_handler,
                handlers::update_resource_limits_docs,
            ),
        )
        // ──────── Flavor catalog (MCP server form picker) ────────
        .api_route(
            "/code-sandbox/flavors",
            get_with(
                handlers::get_sandbox_flavors_handler,
                handlers::get_sandbox_flavors_docs,
            ),
        )
        // ──────── Rootfs versions (Plan 5 Phase 2c) ────────
        .api_route(
            "/code-sandbox/rootfs/versions",
            get_with(
                version_handlers::get_versions_handler,
                version_handlers::get_versions_docs,
            ),
        )
        .api_route(
            "/code-sandbox/rootfs/versions/install",
            post_with(
                version_handlers::install_version_handler,
                version_handlers::install_version_docs,
            ),
        )
        // SSE — live progress for every active install task. Typed
        // via `.api_route` (matches the llm_model/downloads/subscribe
        // pattern) so the frontend's typed API client gets the
        // generated subscriber helper.
        .api_route(
            "/code-sandbox/rootfs/versions/install/subscribe",
            get_with(
                version_handlers::subscribe_install_progress_handler,
                version_handlers::subscribe_install_progress_docs,
            ),
        )
        .api_route(
            "/code-sandbox/rootfs/versions/set-pin",
            post_with(
                version_handlers::set_pin_handler,
                version_handlers::set_pin_docs,
            ),
        )
        .api_route(
            "/code-sandbox/rootfs/versions/{id}",
            delete_with(
                version_handlers::delete_version_handler,
                version_handlers::delete_version_docs,
            ),
        )
}
