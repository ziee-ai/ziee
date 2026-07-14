# Chunk C1 — `ziee-control-mcp` DB-free tool-dispatch core (CUT manifest)

Move the **build-DB-free tool-dispatch core** of ziee's `control_mcp` module
(the LLM-control surface) into a new `ziee-control-mcp` SDK crate, and lift the
**shared built-in-MCP-server scaffolding** (JSON-RPC 2.0 envelope types +
`loopback_host`) into `ziee-framework`. Per decisions **N1/N5** only the
DB-free core moves; the DB-touching + wiring pieces stay app-side (see the
`## Decision` in TRANSFORMS).

## Design gate — the LLM-control surface (v1 = tool-dispatch core only)

`control_mcp` exposes ziee's own 300+ permission-gated REST operations to the
chat model as three MCP tools (`list_capabilities` / `describe_capability` /
`invoke_capability`). Its precision comes from an in-memory catalog built from
the finished `aide::openapi::OpenApi` document; its safety from a
deployment-invariant reachability/mutation policy + a secret-body denylist +
path-param hardening + a per-user permission filter + the forwarded-JWT loopback
re-auth. The **catalog ingest + policy + tool descriptors** are pure functions of
the OpenAPI spec / an `Operation` — no DB, no app types — so they move to the SDK.
The **JSON-RPC handler** (which holds the `RequirePermissions` boundary, the
`User`/`Group` permission filter, the reqwest loopback client, and JWT header
forwarding), the **`mcp_servers`-row upsert** (`repository.rs`), the **route**,
and the **chat_extension** are app-coupled and stay in ziee (v1). A fresh app
self-exposes control only with the Tier-1 `mcp` registry (v1.5, N5).

## Files — MOVED INTO `ziee-control-mcp` (submodule `sdk/`)

- new: `crates/ziee-control-mcp/src/catalog.rs` — the OpenAPI→operation catalog:
  `Operation` / `ControlCatalog`, `init_from_openapi` (the `OnceLock` install),
  the pure `build_catalog`, `extract_path_params`, `parse_required_permission`,
  the recursive secret-field detector (`schema_has_secret_field*`,
  `is_secret_field_name`), `resolve_schema_ref`. Moved verbatim **except** two
  edition-2024 let-chains lowered to nested `if let` for the SDK's edition 2021
  (TRANSFORMS T-2). 7 unit tests move with it.
- new: `crates/ziee-control-mcp/src/policy.rs` — the reachability/mutation policy:
  `is_mutating`, `is_denied`, `DENY_PATH_PREFIXES`, the SSE / token / byte-stream
  segment guards. Moved **byte-for-byte**. 3 unit tests move with it.
- new: `crates/ziee-control-mcp/src/tools.rs` — the three static tool descriptors
  + the tool-name consts (`LIST_CAPABILITIES` / `DESCRIBE_CAPABILITY` /
  `INVOKE_CAPABILITY`) emitted by `tools/list`. Moved **byte-for-byte**.
- edit: `crates/ziee-control-mcp/src/lib.rs` — `pub mod catalog; pub mod policy;
  pub mod tools;` (replaces the placeholder).
- edit: `crates/ziee-control-mcp/Cargo.toml` — add `aide` (catalog ingests
  `OpenApi`), `serde_json`, `tracing`; retain `ziee-framework` + `ziee-identity`
  (plan §1.2 dependency direction; the v1.5 handler/permission-filter extraction
  needs them).

## Files — MOVED INTO `ziee-framework` (submodule `sdk/`)

- new: `crates/ziee-framework/src/mcp.rs` — the shared built-in-MCP-server
  scaffolding: `JsonRpcRequest` / `JsonRpcResponse` / `JsonRpcError` (+ the
  canonical-code constructors + `from_app_error`) and `loopback_host`. Moved
  verbatim **except** `crate::common::AppError` → `ziee_core::AppError` in
  `from_app_error` (TRANSFORMS T-3). The 5 JSON-RPC envelope tests + the 2
  `loopback_host` tests move with it (7 total).
- edit: `crates/ziee-framework/src/lib.rs` — `pub mod mcp;` + `pub use
  mcp::{JsonRpcError, JsonRpcRequest, JsonRpcResponse, loopback_host};`.

## Files — CHANGED IN ziee (submodule `src-app/`, NOT committed here)

- del: `server/src/modules/control_mcp/{catalog,policy,tools}.rs` (moved).
- edit: `server/src/modules/control_mcp/mod.rs` — replaces the three
  `pub mod catalog/policy/tools;` with `pub use ziee_control_mcp::{catalog,
  policy, tools};` so `super::catalog` / `super::policy` / `super::tools` in the
  retained app-side `handlers.rs` and `control_mcp::catalog::init_from_openapi`
  at the two boot sites resolve unchanged.
- edit: `server/Cargo.toml` — add the `ziee-control-mcp` path dep.
- edit: `server/src/modules/code_sandbox/types.rs` — remove the JSON-RPC type
  defs + `default_jsonrpc` + the now-unused `serde::{Deserialize, Serialize}`
  import + the 5 JSON-RPC tests; add `pub use ziee_framework::mcp::{JsonRpcError,
  JsonRpcRequest, JsonRpcResponse};` so every `code_sandbox::types::JsonRpc*`
  importer (13 built-in MCP handlers) resolves unchanged.
- edit: `server/src/modules/code_sandbox/mod.rs` — remove the `loopback_host` fn
  + its doc + the 2 tests; add `pub use ziee_framework::mcp::loopback_host;` so
  every `code_sandbox::loopback_host(...)` caller (15 modules) resolves unchanged.

## Stays app-side (v1 — decisions N1/N5)

`control_mcp/handlers.rs` (the JSON-RPC handler: `RequirePermissions<(ControlUse,)>`
boundary, `user_may_run` per-user permission filter, `substitute_path` /
`validate_body` request hardening, the forwarded-JWT reqwest loopback dispatch,
`control_call_needs_approval`), `control_mcp/repository.rs` (the `mcp_servers` row
upsert — the DB write that makes this NOT build-DB-free), `control_mcp/routes.rs`,
`control_mcp/permissions.rs` (`ControlUse` / `control::use`), and
`control_mcp/chat_extension/*`. These name app types (`User`/`Group`,
`RequirePermissions`, `PgPool`, the app `ModuleContext`) and hold the DB write, so
they remain in ziee until the Tier-1 `mcp` registry exists (v1.5).
