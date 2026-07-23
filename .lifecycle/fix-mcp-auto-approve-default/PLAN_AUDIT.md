# PLAN_AUDIT — fix-mcp-auto-approve-default

Audited against the codebase at `origin/khoi@68af34059`, before writing any code.

## Breakage risk

**Call-site fan-out is tiny — verified by grep, not assumption.**

- `upsert_conversation_settings` (`approval/repository.rs:57`) has exactly **one**
  in-tree caller chain: `approval/handlers.rs:101` → `chat_extension/repository.rs:44`.
  No test, no other module, no desktop crate calls it. Widening its `approval_mode`
  param to `Option<ApprovalMode>` (ITEM-5) touches 3 files and nothing else.
- `upsert_user_defaults` (`defaults/repository.rs:38`) has exactly **one** caller:
  `defaults/handlers.rs:59`. Same for ITEM-6.
- `get_user_defaults` has 3 callers: `defaults/handlers.rs:36`, `mcp.rs:2581`, and
  `workflow/dispatch.rs:1162`. The workflow caller uses **only**
  `get_disabled_servers()` — it never reads `get_approval_mode()` — so ITEM-2 cannot
  change scheduled/standalone workflow behavior.
- `get_conversation_settings` callers: `approval/handlers.rs:62` and `mcp.rs:2339`
  (the single fetch reused by the resolve block). Not re-signatured.

**Is `ApprovalMode`'s `Default` impl load-bearing today?** No. `#[derive(Default)]` +
`#[default] ManualApprove` exist at `approval/models.rs:13-21` but nothing consumes
them: no `unwrap_or_default()` on an `ApprovalMode`, no `..Default::default()` on a
struct holding one. The four structs with an `approval_mode` field
(`ConversationMcpSettingsResponse`, `UpsertMcpSettingsRequest`,
`UserMcpDefaultsResponse`, `UpsertUserMcpDefaultsRequest`, plus
`ProjectMcpSettingsRequest` and `js_tool::executor`'s request) **none derive `Default`**
— checked individually. So ITEM-1..4 promote a currently-dead impl into the single
source of truth. That is the point, but it means the blast radius is exactly the sites
we deliberately route through it — nothing inherits it silently.

**Does `#[serde(default)]` on an `Option` field surprise us?** No.
`#[serde(default)]` on `Option<ApprovalMode>` yields `None`, not
`ApprovalMode::default()` — the "absent" signal stays distinguishable from an explicit
value, which is what the COALESCE needs. (Contrast: `#[serde(default)]` on a bare
`ApprovalMode` WOULD silently inject the default and defeat the COALESCE; we are not
doing that.)

**Wire compatibility.** Making a required request field optional is additive: every
existing client that sends `approval_mode` behaves byte-identically. No client is
forced to change. The project-scope PUT (`project_extension/handlers.rs:114`) keeps
sending `Some(...)` and is untouched.

**js_tool coupling — checked.** `js_tool::executor`'s `approval_mode`
(`executor.rs:44,330`) is fed from the mcp chat extension's resolved value via
`execute_run_js_call(..., approval_mode: &ApprovalMode, ...)` (`mcp.rs:682`). So
ITEM-4's `resolve_approval` is also the source for the `run_js` gate — the fix stays
consistent across both gates instead of diverging. `js_tool/approval.rs::gate` itself
is untouched (it takes the mode as a parameter).

**Non-goals honored.** `is_builtin_server_id` / `auto_attach_builtin_ids` and
`usage_mode` are not in *Files to touch*.

## Pattern conformance

- **ITEM-5/6 COALESCE** — the target query ALREADY does exactly this for the sibling
  field: `approval/repository.rs:66-70` builds `serde_json::Value::Null` for "absent",
  `:82` inserts `COALESCE($4, '[]'::jsonb)`, `:86` updates
  `COALESCE($4, mcp_settings.auto_approved_tools)`. ITEM-5 adds the identical shape one
  column over, inside the same statement. Highest-fidelity precedent available.
- **ITEM-4 pure extraction** — matches `chat/extensions/project/project.rs`'s
  `apply_project_context` (a pure fn lifted out of an extension hook solely for
  unit-testability) and the in-source `#[cfg(test)]` convention used by
  `mcp/tool_calls/record.rs`, `js_tool/approval.rs:190+`.
- **ITEM-7 additive response field** — mirrors the existing
  `UserMcpDefaultsGetResponse { defaults: Option<..> }` /
  `McpSettingsResponse { settings: Option<..> }` shape; a sibling scalar alongside the
  Option is the least-surprising extension. Deliberately NOT synthesizing a fake
  `defaults` row (would fabricate `id`/`user_id`/`created_at`, flip
  `McpInitializer.tsx:39`'s `if (userDefaults)` branch into
  `applyUserDefaultsToPending`, and break the existing assertion
  `mcp_defaults_test.rs:64`).
- **ITEM-9 pure frontend module** — `ui/src/modules/mcp/stores/approvalRouting.ts` is
  the exact precedent (enum-free, store-free, importable standalone, already
  re-exported through `McpComposer.store.ts:92-96`), and
  `chat-extension/toolRun.test.ts` is the `node:test` companion pattern.
- **Tests** — `tests/mcp/mcp_defaults_test.rs` is the model: bare `reqwest` helper fns,
  `test_helpers::create_user_with_permissions`, `json!` payloads, `TestServer::start()`.

## Migration collisions

**None — this feature ships no migration.** Migrations are per-module
(`modules/<mod>/migrations/`); the mcp module's are
`202607140180_mcp_schema.sql`, `…4170_mcp_fkeys.sql`, `…5045_mcp_seed.sql`,
`…6065_mcp_grant_permissions.sql`. Nothing is added, so no number can collide.

The `approval_mode` column defaults (`202607140180_mcp_schema.sql:56` for
`mcp_settings`, `:132` for `user_mcp_defaults`) remain `manual_approve` on both
branches. Verified unreachable: every INSERT touching those columns names
`approval_mode` explicitly (`approval/repository.rs:79-82`,
`defaults/repository.rs`, `settings/repository.rs:155-159,196-200,305-309,330-334`,
`project_extension/extension.rs:81`). ITEM-3 records this in a comment rather than
shipping a schema change.

No seed SQL, no backfill SQL, no mass UPDATE/DELETE — per the user-confirmed
constraint. Every write remains scoped to the single upserted row.

## OpenAPI regen

**Required, both workspaces.** Three schema deltas:
`UpsertMcpSettingsRequest.approval_mode` and
`UpsertUserMcpDefaultsRequest.approval_mode` become optional; new
`UserMcpDefaultsGetResponse.default_approval_mode`.

Run `just openapi-regen` (justfile:521-525) — it runs the `ziee` binary for
`src-app/ui/openapi` AND the `ziee-desktop` binary for `src-app/desktop/ui/openapi`,
each emitting both `openapi.json` and `src/api-client/types.ts`. Do NOT `cp` the
server spec onto the desktop one (justfile:519-520). The
`openapi::emit_ts::tests::types_ts_parity` golden test is the enforcement.

**R2-3 desktop-override check:** `src-app/desktop/ui/src/modules/` contains
`chat, desktop-base, file-dialog, host-mount, layouts, memory, remote-access,
tunnel-auth, updater, window` — there is **no** `mcp` module there, so ITEM-9..12 have
no hand-written desktop counterpart to keep in sync. Desktop's only exposure is the
regenerated `api-client/types.ts` + `openapi.json`.

**Frontend-gate consequence:** the generated `openapi.json`/`api-client/types.ts` are
excluded from the lifecycle's UI-diff detection, but ITEM-9..12 touch real
`src-app/ui/src/modules/mcp/**` source — so this **is** a UI-touching diff and the
phase-3 e2e requirement + phase-8 `npm run check` / `gate:ui` gates apply.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — `#[derive(Default)]` + `#[default] ManualApprove` already exist at `approval/models.rs:13-21` and are currently unconsumed; documenting + promoting them adds no new type and no new dependency.
- **ITEM-2** — verdict: PASS — `.unwrap_or_default()` is byte-equivalent to the current `.unwrap_or(ApprovalMode::ManualApprove)` on khoi. The unwrap only fires on a non-parseable DB string (`FromStr` rejects anything outside the 3 known values), which no writer can produce.
- **ITEM-3** — verdict: CONCERN — `DEFAULT_APPROVAL_MODE` is currently a `const &str`; `ApprovalMode::default().to_string()` is not const-evaluable, so it must become a small `fn default_approval_mode() -> String` (or a `LazyLock`). Mechanical, but the const→fn shape change touches its 2 use sites (`settings/repository.rs:104,131`). Resolve during implementation; no plan change.
- **ITEM-4** — verdict: PASS — the block at `mcp.rs:2591-2604` has no side effects and depends only on `settings` + `user_defaults`, both already materialized above it; a pure extraction is safe. Its output also feeds `execute_run_js_call` (`mcp.rs:682`), so the js_tool gate inherits the same resolution — intended, not a regression.
- **ITEM-5** — verdict: PASS — single caller chain (handlers → chat_extension/repository → approval/repository); COALESCE precedent is in the same statement for `auto_approved_tools`.
- **ITEM-6** — verdict: PASS — single caller (`defaults/handlers.rs:59`). This is the item that closes the `McpStatusRow.tsx:69` chip-removal clobber, which the original bug report did not identify.
- **ITEM-7** — verdict: CONCERN — needs `just openapi-regen` for BOTH workspaces (new response field). Also must not break `mcp_defaults_test.rs:64`'s `defaults` is-null assertion — satisfied because `defaults` stays `Option` and the new field is a sibling. Covered by TESTS.
- **ITEM-8** — verdict: CONCERN — `just openapi-regen` depends on `check-hub` and runs two `cargo run` binaries; budget the build time. The `types_ts_parity` golden test fails loudly if skipped, so this cannot be silently dropped.
- **ITEM-9** — verdict: PASS — new file, no existing importer; mirrors `approvalRouting.ts` exactly.
- **ITEM-10** — verdict: PASS — all 9 literal sites are inside `McpComposer.store.ts` and construct the same `ConversationMcpConfig` shape (`:74-85`); a single helper covers them without changing the type.
- **ITEM-11** — verdict: CONCERN — `saveProjectConfig` (`:485`) PUTs to `/projects/{id}/mcp-settings`, whose request type `ProjectMcpSettingsRequest.approval_mode` (`project_extension/models.rs:25`) stays REQUIRED under this plan. So ITEM-11's "omit when absent" applies only to the two conversation/user-defaults PUTs; for the project PUT the value must still be sent. Narrow ITEM-11 at implementation time to `saveConversationConfig` + `saveUserDefaults`, and have `saveProjectConfig` send the server default rather than the `'manual_approve'` literal. Recorded as DEC in phase 4.
- **ITEM-12** — verdict: PASS — both fallback branches (`extension.tsx:1092-1098`, `:1111-1117`) build the same config literal and are reached only when the conversation has no stored settings or the GET failed; using the server default there is strictly more correct.

- **ITEM-13** — verdict: PASS — `McpConfigModal.tsx:85` is a pure display fallback feeding the radio at `:401`; it has no write path, so correcting it cannot alter what gets persisted, only what the user is told. Verified that the sibling `ProjectMcpSettingsPanel.tsx:44` fallback is genuinely unreachable (`project_extension/handlers.rs:42` uses `get_or_default`, which always returns a row) and is therefore correctly left alone rather than churned.

**No BLOCKED verdicts.** The three CONCERNs (ITEM-3 const→fn, ITEM-7/8 regen,
ITEM-11 project-PUT scope) are all resolvable in-flight and are carried into
DECISIONS.md rather than amending the item list.
