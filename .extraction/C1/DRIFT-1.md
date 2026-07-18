# Chunk C1 — DRIFT scan (round 1)

Drift = any place moving the control tool-dispatch core + the shared MCP
scaffolding could diverge from pre-extraction behavior / surface / output. Each
candidate reconciled below.

- **DRIFT-1.1** — verdict: none. **policy/tools byte-identity.** `policy.rs` +
  `tools.rs` copied then `diff <(git show HEAD:…) sdk/…` is empty (exit 0). No
  logic/text change. The 3 policy tests pass in the SDK.

- **DRIFT-1.2** — verdict: none. **catalog semantic identity.** Only two
  edition-2024 let-chains were lowered to nested `if let` (edition-2021 compat) —
  the exact desugaring of a 2-term let-chain, so the secret-field recursion is
  behaviorally identical. `detects_secret_request_field` (which drives the
  `items`/`anyOf`/`oneOf`/`allOf` paths those chains implement) passes. `diff`
  shows only those two edits across ~552 lines.

- **DRIFT-1.3** — verdict: none. **JSON-RPC type identity.** The three envelope
  types + constructors moved verbatim; the single production edit is
  `crate::common::AppError` → `ziee_core::AppError` in `from_app_error`, and in
  ziee `crate::common::AppError` IS a re-export of `ziee_core::AppError`, so the
  `status_code()`/`error_code()`/`to_string()` surface + the exact 400/403/404/
  409/410/422 → invalid_params mapping is unchanged. The 5 envelope tests pass in
  the framework.

- **DRIFT-1.4** — verdict: none. **`loopback_host` security invariant.** Moved
  verbatim (`pub fn loopback_host(_server_host: &str) -> &str { "127.0.0.1" }`) —
  still ignores its argument and always returns loopback, so no config-set
  `server.host` can redirect a built-in server's JWT-bearing self-dial off-host.
  Both security regression tests (`…_for_wildcards`, `…_regardless_of_server_host`)
  move with it and pass in the framework.

- **DRIFT-1.5** — verdict: none. **Shim transparency (control core).**
  `control_mcp/mod.rs`'s `pub use ziee_control_mcp::{catalog, policy, tools};`
  keeps `super::catalog/policy/tools` (in the retained `handlers.rs`) +
  `control_mcp::catalog::init_from_openapi` (the two boot sites) resolving. `ziee`
  + `ziee-desktop` compile (exit 0); zero call-site edits outside `mod.rs` +
  `Cargo.toml`.

- **DRIFT-1.6** — verdict: none. **Shim transparency (JSON-RPC + loopback).**
  `code_sandbox::types` re-exports the three JSON-RPC types; `code_sandbox::mod`
  re-exports `loopback_host`. All 13 `code_sandbox::types::JsonRpc*` importers +
  all 15 `code_sandbox::loopback_host(...)` callers resolve unchanged (proved by
  the ziee build). The now-unused `serde::{Deserialize, Serialize}` import was
  dropped so `-D warnings` stays clean.

- **DRIFT-1.7** — verdict: none. **Golden output (E8, BOTH surfaces).** These
  moves touch no route registration, request/response schema, permission string,
  or OpenAPI-visible type (the control route uses `.route()` not `api_route`; the
  JSON-RPC types are Deserialize/Serialize only, never `JsonSchema`). Regenerated
  ui + desktop: `types.ts` **byte-identical** (both) via `cmp`; `openapi.json`
  **canonically-equal** (both) via `diff <(jq -S)` — only linkme dep-graph
  key-order churn appears (E8 REFINEMENT). Restored via `git checkout`.

- **DRIFT-1.8** — verdict: none. **`control_call_needs_approval` still governs
  the model's writes.** It stays in the app-side `handlers.rs`, calling the
  re-exported `policy::is_mutating` + `catalog::catalog()`. `mcp/chat_extension/
  mcp.rs:2360` + `js_tool/executor.rs:149` call it unchanged — the "mutating
  invoke always needs approval" posture is preserved (ziee build green; the 5
  approval tests stay in the app-side handler).

- **DRIFT-1.9** — verdict: none. **Build hygiene / boundary.** `ziee-control-mcp`
  adds `aide`/`serde_json`/`tracing`; retains the (currently-unused-in-v1)
  `ziee-framework`/`ziee-identity` deps per plan §1.2 — no `-D warnings` failure
  (unused crate deps are allow-by-default). No build DB introduced (catalog reads
  the in-memory spec; the `mcp_servers` write stayed in ziee). `cargo check
  --workspace` exit 0. The 4 `ziee (lib)` dead-code warnings
  (`KB_MAX_DOCUMENTS_DEFAULT`, `is_unread`, `is_active`,
  `list_enabled_for_health_check`) pre-date this chunk.

**Unresolved drifts: 0**
