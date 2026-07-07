# Office Bridge — PLAN_AUDIT

Audited against the worktree at `origin/main` (ca634b98). Facts verified: modules are declared as
`pub mod <name>;` in `src-app/server/src/modules/mod.rs`; `MODULE_ENTRIES` order **97 is unused**
(96=web_search, then 100/102/103); `mcp/chat_extension/mcp.rs` has `auto_attach_builtin_ids` (L121)
and `is_builtin_server_id` (L229); `SyncEntity` lives in `modules/sync/event.rs`;
`code_sandbox/backend/mod.rs` exposes `trait SandboxBackend` (L62) + `pub fn active()` (L255);
migration ceiling is **131** (132/133 free); `rcgen`, `axum-server`, the `windows` crate, and axum's
`ws` feature are **not yet present** (axum 0.8.4 has only `macros,multipart`).

## Breakage risk
No item modifies an existing caller's contract except the single additive edit in
`mcp/chat_extension/mcp.rs::auto_attach_builtin_ids` (append a branch — same shape as web_search/bio/
lit) and one additive `SyncEntity` variant (enum grow → regen). The module is `probe()`-gated so on a
headless/Linux server it never registers, spawns no listener, and adds no runtime surface (mirrors
`code_sandbox` probe_host). New crate deps (`rcgen`, `axum-server`, `rustls`, `windows`, axum `ws`)
are additive; the `windows` crate is `#[cfg(windows)]`-scoped so non-Windows builds are unaffected.
Port 44300 is only bound when `probe()` succeeds (desktop + Office present).

## Pattern conformance
Every item names its mirror (see PLAN "Patterns to follow") and those mirrors all exist on current
main: `web_search/` (module skeleton + JSON-RPC + mcp.rs edit + permissions + grant migration),
`code_sandbox/backend/mod.rs` (seam), `llm_local_runtime/proxy.rs` (token), `skill/builtin.rs`
(`include_dir!`), `sync/` (sync_publish + SyncEntity in event.rs), `literature/` frontend tri (now
shadcn). Frontend obeys `components/ui` KIT_MANIFEST/TOKEN_MAP + DESIGN_SYSTEM.md + the
`frontend-ui-engineering`/`shadcn-component-*` skills.

## Migration collisions
Ceiling is 131; ours are `132_create_office_bridge.sql` and `133_grant_office_bridge_permissions_to_users.sql`
— no collision. Grant migration uses the idempotent `DO $$` Users-group `array_append` pattern from
`…098_grant_web_search_permissions_to_users.sql`.

## OpenAPI regen
Items adding `#[derive(JsonSchema)]` DTOs (ITEM-1 models, ITEM-13 connect response), new permissions
(ITEM-3), and the new `SyncEntity` variant (ITEM-11) require `just openapi-regen` (both `ui/` and
`desktop/ui/`); ITEM-15 covers it and the golden `types_ts_parity` test must stay green. Generated
`openapi.json`/`types.ts` are excluded from the blind-audit coverage law and don't count as UI touch.

## Per-item verdicts
- **ITEM-1** — verdict: PASS — direct mirror of `web_search/mod.rs` (server-id v5, `#[distributed_slice(MODULE_ENTRIES)]` order 97 free, init kill-switch + spawned idempotent upsert, register_routes merge); probe-gating adds the code_sandbox idiom.
- **ITEM-2** — verdict: PASS — singleton-settings + grant migrations at free numbers 132/133, mirroring web_search 097/098; no collision.
- **ITEM-3** — verdict: PASS — compile-time `PermissionCheck` impls mirroring `web_search/permissions.rs`; `use` granted via migration 133.
- **ITEM-4** — verdict: CONCERN — `rcgen` is a new dep (additive, low risk); must ensure SAN includes `::1` (the proven load-bearing detail) and cert is cached/reused so the trusted cert stays stable across restarts.
- **ITEM-5** — verdict: CONCERN — new standalone rustls WSS listener (axum `ws` + `axum-server`/`rustls` new deps) is novel to ziee (no existing axum WS); dual-stack bind + Origin allowlist + token gate must be right. Isolated (probe-gated), so blast radius is contained; verified by ITEM-5 integration tests.
- **ITEM-6** — verdict: PASS — structural copy of `code_sandbox/backend/mod.rs` (`#[async_trait]` trait + `#[cfg]` `ACTIVE` + `active()`), with a mock impl for tests like `MockTunnelDriver`.
- **ITEM-7** — verdict: CONCERN — the `windows` crate (late-bound IDispatch/`GetActiveObject`/oleacc/EnumWindows) is the largest new surface and the trickiest code; all `#[cfg(windows)]`-scoped so it can't break other targets, and each COM call maps to a spike-proven operation. Elevated `certutil` needs one UAC (approved).
- **ITEM-8** — verdict: PASS — `#[cfg(target_os="macos")]` scaffold + `unsupported` fallback; explicitly UNVERIFIED behind `MAC_TRANSPORT_VERIFIED=false`, so it compiles without claiming correctness (per the plan's macOS gate).
- **ITEM-9** — verdict: PASS — JSON-RPC dispatch mirroring `web_search/handlers.rs`; capability-gating returns typed errors for unsupported host ops (PPT comments/track-changes), matching the proven capability matrix.
- **ITEM-10** — verdict: PASS — chat-extension flag + `#[distributed_slice(CHAT_EXTENSIONS)]` order 29 (<30) + the single `auto_attach_builtin_ids` edit; `is_builtin_server_id` intentionally NOT edited so the mutating tool stays behind approval (matches `control_mcp`'s deliberate absence).
- **ITEM-11** — verdict: CONCERN — adding a `SyncEntity::OfficeDocument` variant in `sync/event.rs` forces an OpenAPI regen and an explicit `Audience::owner` choice at the emit site (never `everyone()` for per-user data); the daemon poll-diff watcher is new but self-contained.
- **ITEM-12** — verdict: PASS — `include_dir!("$CARGO_MANIFEST_DIR/resources/office-bridge")` per `skill/builtin.rs`; assets ported from the proven spike manifest/taskpane.
- **ITEM-13** — verdict: PASS — an admin-gated REST action + settings button; elevation/Office-present detection reuses ITEM-7 platform methods.
- **ITEM-14** — verdict: CONCERN — frontend is shadcn/Radix now (not the antd of prior art); must mirror the CURRENT `literature` panel tri + obey KIT_MANIFEST/design skills; requires ≥1 e2e spec (enumerated in TESTS.md) + `npm run check`/`gate:ui`.
- **ITEM-15** — verdict: PASS — `just openapi-regen` after DTO/permission/sync additions; golden `types_ts_parity` kept green; generated artifacts excluded from coverage law.
