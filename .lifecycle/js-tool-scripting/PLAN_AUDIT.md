# PLAN_AUDIT — js-tool-scripting

Audited against the worktree (`origin/main`, `cargo check -p ziee` green at
baseline). Five subsystem studies grounded this: built-in MCP pattern, dispatch
chokepoint + approval + recording, chat execution loop + code_sandbox contrast,
rquickjs crate maturity, frontend tool-call/approval rendering.

## Breakage risk
- **Execution intercept (ITEM-8)** is additive: the loop only diverts when
  `server_id == run_js_mcp_server_id()`; every other tool keeps the generic
  `execute_tool` path. Risk is placing it at the wrong site — the loop has TWO
  execute sites (`mcp.rs:2311` non-sampling, `:2226` sampling). run_js is invoked
  by the model in the main after_llm_call loop, so the non-sampling site is the
  intercept; a run_js call arriving on a sampling session is out of scope and
  falls through to the loopback handler's explicit error (ITEM-6) rather than
  mis-executing. Confirm the exact site holds `session_manager` + the assembled
  accessible-servers list in scope at impl time.
- **`source='script'` insert (ITEM-9)**: `mcp_tool_calls.source` has a live
  `CHECK (source IN ('chat','rest','always','sampling','approval','workflow'))`
  (migration 105, widened by 108). Without migration 133 the recording insert
  fails — but the insert is fire-and-forget (`tokio::spawn` in `record_call`), so
  a missing widen degrades to a logged warning, not a failed tool call. Migration
  133 is in-plan; this is the mandatory ALTER, not optional.
- **Default-enabled blast radius (ITEM-10)**: `js_tool.enabled=true` injects a
  `run_js` tool into EVERY tool-capable conversation, changing the tool set every
  model sees. Must verify no existing chat/integration test hard-asserts an exact
  tool list/count; if one does, gate the default in the test harness. Additive to
  the wire tool list; no signature change to existing tools.
- **Reusing `mcp::elicitation::registry` for approvals (ITEM-4/ITEM-13)**: the
  registry is keyed by a random per-request id, so run_js approvals cannot collide
  with real `ask_user` elicitations. Risk: the full `run_ask_user_elicitation`
  path also persists a pending DB *content* row and assumes a form schema — we
  want the registry + `respond` endpoint but NOT necessarily the content-row
  persistence. Resolve the exact reuse boundary in DECISIONS (DEC-2): reuse the
  registry + `POST …/respond` resolver + owner-binding; carry a distinct
  render-hint; skip form-content persistence for approvals.
- **rquickjs C-compile (ITEM-1)**: adds a C compilation of QuickJS-NG to the
  build. Pre-generated bindings ship for Linux/macOS/Windows (no bindgen/clang),
  and a C compiler is already a documented host dep (pgvector's `make`+`gcc`), so
  Linux is covered; the mac + windows build hosts must be re-verified (they build
  the server). No effect on downstream crates.

## Pattern conformance
- **ITEM-6/ITEM-7** mirror `memory_mcp/` + `web_search/chat_extension/`
  faithfully (deterministic id, boot upsert, attach flag, two mcp.rs edits). High
  conformance — the checklist is well-trodden.
- **ITEM-2/3/4/5** are a NOVEL subsystem (no existing embedded-interpreter
  module). Closest analogs: `run_ask_user_elicitation` (in-process oneshot
  suspend/resume) for approval, `code_sandbox` (a built-in that executes) for the
  server shape, and the mcp.rs execute path for dispatch re-entry. Mitigation:
  keep `runtime.rs` PURE (no chat context) so it is exhaustively unit-testable,
  and localize all context-bearing logic in `executor.rs`/`host_bridge.rs`.
- **ITEM-8 intercept site** deviates from the `ask_user` precedent (which
  intercepts inside `execute_tool` in helpers.rs); run_js must intercept one level
  up in mcp.rs because it needs `session_manager` + accessible-servers +
  approval channel, which `execute_tool` does not receive. Documented divergence
  with rationale; not a violation.
- **System nudge (ITEM-7)** must be GENERIC (point the model at a runtime
  `ziee.toolList()` for exact bindings) rather than enumerating specific tool
  names, because the concrete binding set is computed at execution time in the
  executor, after tool assembly in mcp(30) — so the nudge cannot know the names
  when it runs at order 29. This removes any ordering dependency.

## Migration collisions
- Highest existing migration is `00000000000132`. Plan adds `133`
  (`mcp_tool_calls_source_script`) and `134` (`grant_js_tool_permissions_to_users`)
  — no collision. `133` mirrors `108`'s DROP+ADD CONSTRAINT shape exactly; `134`
  mirrors `098`/`104`'s idempotent `DO $$` grant to the system `Users` group.
- No new table, so no new SQLx query-verification surface beyond the upsert
  (mirrors `memory_mcp/repository.rs`, already verified shape).
- Chat-extension `order = 29` ties `citations` (also 29); the sort is ascending
  and ties are benign here (both are pre-mcp flag-setters with independent flags;
  the module-order already tolerates ties, e.g. control/workflow at 88). No
  ordering correctness dependency after the generic-nudge fix above.

## OpenAPI regen
- The `run_js` loopback JSON-RPC route uses plain `axum::routing::post` (like
  every other built-in MCP endpoint), so it is deliberately absent from OpenAPI —
  no schema entry.
- `McpToolCallSource::Script` IS surfaced (the enum is returned by
  `GET /api/mcp/tool-calls`), so it flows into `api-client/types.ts`. Any new
  elicitation render-hint field added to the shared elicitation request/response
  types also flows through. Therefore `just openapi-regen` MUST run for BOTH
  binaries (server → `ui/`, desktop → `desktop/ui/`); `npm run check` in both
  workspaces must be green (the `types_ts_parity` golden test enforces the server
  side). Captured as ITEM-15.
- The elicitation `POST /api/mcp/elicitation/{id}/respond` endpoint already exists
  in the spec — reusing it adds no new REST route.

## Per-item verdicts
- **ITEM-1** — verdict: CONCERN — `rquickjs 0.12.1` (`macro`+`futures`, default allocator, no `parallel`) meets every hard requirement; adds a QuickJS-NG C compile — re-verify the mac/windows build hosts. Resolve the version/feature resolution early in phase 5 via `cargo check`.
- **ITEM-2** — verdict: PASS — pure embedded-runtime wrapper; caps map to verified `AsyncRuntime` APIs (`set_interrupt_handler`/`set_memory_limit`/`set_max_stack_size`); default allocator kept so the memory cap is live.
- **ITEM-3** — verdict: PASS — host-fn calls re-enter `get_or_create_with_context`→`execute_tool` with `sse_tx=None` (Option-typed, tolerated); recording is automatic on the stamped session.
- **ITEM-4** — verdict: CONCERN — in-process oneshot suspend/resume is the correct (only feasible) analog; the exact reuse boundary of the elicitation registry vs. its content-persistence is resolved in DEC-2.
- **ITEM-5** — verdict: PASS — executor reuses `validate_and_build_config` + `auto_attach_builtin_ids` for the tool set; active-execution deadline excludes approval-wait (interrupt = CPU, tokio timeout = wall-clock).
- **ITEM-6** — verdict: PASS — mirrors `memory_mcp/` verbatim; reuses `code_sandbox::types` envelope; loopback `tools/call` returns an explicit "invoke in chat context" error (like `ask_user`).
- **ITEM-7** — verdict: PASS — mirrors `web_search/chat_extension/`; nudge made generic (points to `ziee.toolList()`), removing any order dependency.
- **ITEM-8** — verdict: CONCERN — the two mcp.rs edits are boilerplate; the execution intercept is a documented divergence from the `ask_user`-in-`execute_tool` precedent — confirm the intercept site has `session_manager` + accessible-servers in scope at impl.
- **ITEM-9** — verdict: PASS — `Script` variant + `as_str`; migration 133 ALTERs the CHECK constraint mirroring migration 108 exactly.
- **ITEM-10** — verdict: CONCERN — `js_tool.enabled` default `true` mirrors web_search/lit_search but has a wider blast radius (injects run_js everywhere); verify no test hard-asserts tool counts and confirm default-on with the user (DEC-4).
- **ITEM-11** — verdict: PASS — result is a standard `McpContentData::ToolResult` with capped + `ziee://`-scrubbed `structured_content`; only the final value reaches context; error carries line for one-shot self-correction.
- **ITEM-12** — verdict: PASS — in-process + cross-platform, so enabled on desktop (no blocklist), consistent with the embedded-interpreter rationale.
- **ITEM-13** — verdict: CONCERN — reuses `ToolCallPendingApprovalContent` visual + `resolveElicitation`; depends on DEC-2's approval-mechanism resolution; must land in both ui workspaces.
- **ITEM-14** — verdict: PASS — one-line source tone + gallery deep-states mirror the existing `deep-chat-tool-approval` cell to satisfy state-matrix/gallery-coverage.
- **ITEM-15** — verdict: CONCERN — mandatory `just openapi-regen` for BOTH binaries; `npm run check` green in both workspaces (golden parity test enforces server side).

## Admin-configurable limits increment (ITEM-16..27) — audit vs codebase

Reviewed against the `code_sandbox` §6 reference (the exact pattern this mirrors),
the `declare_repositories!` macro, the `SyncEntity` enum, the permission→TS-enum
generation, and the tokio version (1.52.3 → `add_permits`+`forget_permits` present).

- **ITEM-16** — verdict: PASS — singleton-table shape is copied field-for-field from migration 41; next free number is 135 (`ls migrations` → highest is 134); admin-only, so no grant migration (matches migration 41's trailing note).
- **ITEM-17** — verdict: PASS — `JsToolSettings`/`UpdateJsToolSettings`/`validate()` mirror `resource_limits.rs`; DB columns are `i64`/`i32`, converted to `usize`/`u64`/`Duration` in the mapping (ITEM-20), so no type mismatch.
- **ITEM-18** — verdict: PASS — `impl JsToolRepository { get/update_settings }` mirrors the code_sandbox impl-in-settings-file idiom; adding `js_tool` to `declare_repositories!` (repository.rs:198-229) is a one-line addition; `JsToolRepository::new(pool)` already exists.
- **ITEM-19** — verdict: PASS — `settings_cache.rs` mirrors `resource_limits_cache.rs` (`OnceLock<RwLock<Arc>>`); no new dep.
- **ITEM-20** — verdict: CONCERN — extending `JsCaps` with 2 fields touches its only constructor sites (`Default` + the new `from_settings`); `mcp.rs:437` is the sole external `JsCaps::default()` caller (ITEM-22 changes it). No other caller breaks (verified by grep — only executor reads `req.caps.*`).
- **ITEM-21** — verdict: CONCERN — replacing the `static GLOBAL_RUN_SEM: Semaphore` with an `OnceLock<Semaphore>` changes the admission-acquire site (executor.rs:257); the live-resize is a NEW idiom (code_sandbox re-creates per-boot instead), but `add_permits`/`forget_permits` are API-supported in tokio 1.52.3. Shrink is best-effort (in-flight runs keep their slot) — acceptable + documented. Promoting `MAX_CONCURRENT_DISPATCH`/`MAX_TRACE_ENTRIES` consts to `JsCaps` fields is mechanical.
- **ITEM-22** — verdict: PASS — one call-site swap (`JsCaps::default()` → `JsCaps::from_settings(&settings_cache::get().await?)`); `execute_run_js_call` is already `async` and already returns a `Result`, so the `?` on the cache read fits.
- **ITEM-23** — verdict: PASS — permission structs mirror `code_sandbox/permissions.rs`; they surface to TS only via the `with_permission` docs in ITEM-24 (accounted for) + regen (ITEM-27).
- **ITEM-24** — verdict: CONCERN — handlers/routes/docs mirror code_sandbox exactly; requires `just openapi-regen` (new operations) — folded into ITEM-27. `sync_publish` + `Audience::perm::<JsToolSettingsRead>()` + `Uuid::nil()` match the code_sandbox emit site.
- **ITEM-25** — verdict: PASS — one `SyncEntity` variant; `snake_case` → `js_tool_settings`; TS union + EventBus key auto-generate (no manual TS edit) — matches how `CodeSandboxSettings` works.
- **ITEM-26** — verdict: CONCERN — new `ui` frontend module mirroring `modules/code-sandbox/`; UI-only (no `desktop/ui/modules/code-sandbox` exists, confirmed), so no desktop module. Introduces new render states (loaded/loading/no-permission) → needs gallery/state-matrix coverage or an allowlist entry (budgeted in TESTS as an e2e + `npm run check`).
- **ITEM-27** — verdict: CONCERN — mandatory `just openapi-regen` for BOTH binaries; `types_ts_parity` golden test enforces the server side; `npm run check` in `ui` (and desktop/ui only carries the regen). Same regen discipline as ITEM-15.
