# PLAN_AUDIT — mcp-toolcollect-timeout

Plan audited against the codebase before implementation.

## Breakage risk

Low. Every change WRAPS an existing await in `tokio::time::timeout` and routes the
elapsed case into an already-existing `Err`/`warn+continue` arm. The success path is
byte-identical (the inner future resolves before the budget). The inner stdio helper
returns the SAME `errors::upstream_error(name, Unreachable, msg)` value the existing
`serve()`-failure arms return, so `McpSession::new` → `create_session_tracked` →
`record_connection_failure` see no new error variant. No caller signature changes.

## Pattern conformance

- Timeout budget mirrors `http.rs:1097` (`server.timeout_seconds.max(1) as u64`) —
  the same tunable already governs the HTTP client's overall timeout, so behavior is
  consistent across transports.
- Error shape reuses the three existing `stdio.rs` `upstream_error(..., Unreachable, …)`
  call sites (231/283/296) verbatim.
- Test stub reuses `MockMcpServer::DelayedJsonOk` (proven in `tool_call_timeout_test.rs`).
- Breaker tests extend the existing `manager.rs::mod breaker_tests`.

## Migration collisions

None — this branch adds no migration. Highest server prefix `202608250100`, no dups.

## OpenAPI regen

Not required — no handler signature / response type / schema change. `openapi.json` +
`api-client/types.ts` unchanged for both ui and desktop.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — auto-mode site confirmed at mcp.rs:2431-2464; existing
  `warn!(… "— skipping"); continue;` arm at 2446-2453 is the reuse target. `server`
  (redacted McpServer) carries `timeout_seconds: i32` (models.rs:215).
- **ITEM-2** — verdict: PASS — always-mode site confirmed at mcp.rs:2270-2398; three
  existing `warn!` arms (2304/2326/2387) are the reuse targets; per-tool `call_tool`
  at 2362. Timeout wraps each without changing collected-context semantics.
- **ITEM-3** — verdict: PASS — `connect_native` serve() at stdio.rs:204; error arm at
  205-240 shows the exact `upstream_error(Unreachable)` shape to reuse on elapse.
- **ITEM-4** — verdict: PASS — `connect_sandboxed` serve() at stdio.rs:283 (LinuxBwrap)
  and 296 (VM); both already map serve() err → `upstream_error(Unreachable)`.
- **ITEM-5** — verdict: PASS — a single `pub(super)` helper in stdio.rs avoids a magic
  number and gives INV-1 a directly-unit-testable seam (mirrors `filter_env` being
  `pub(super)` for the same reason). `tokio` (with `time`) is already a dependency.

No BLOCKED verdicts.
