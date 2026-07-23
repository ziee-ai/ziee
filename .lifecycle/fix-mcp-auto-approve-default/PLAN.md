# PLAN — fix-mcp-auto-approve-default

Bugfix: MCP auto-approve does not survive past turn 1 of a conversation.

On the deployed BioGnosia instance the `biognosia` `query_rag` tool is auto-approved
when the FIRST message triggers it, but a LATER message that needs it prompts for
permission. Root cause (verified in-tree):

1. Auto-approve on deploy exists ONLY as a no-row fallback —
   `mcp/chat_extension/mcp.rs:2591-2604` branch C returns
   `(ApprovalMode::AutoApprove, [])` when a conversation has no `mcp_settings` row.
2. Right after the first send the shared frontend WRITES a row:
   `ui/.../mcp/chat-extension/extension.tsx:1168-1184` → `saveConversationConfig`
   → `McpComposer.store.ts:409` PUTs `approval_mode: config.approvalMode || 'manual_approve'`,
   and for a fresh conversation `config.approvalMode` is the store's hardcoded
   `'manual_approve'` (`McpComposer.store.ts:321`). The frontend never learned the
   server's default because `GET /api/mcp/defaults` returns `null` when the user has
   no defaults row, and there is no other channel carrying the server default.
3. Turn 2+ therefore takes branch A with the stored `manual_approve` + an empty
   allow-list → approval prompt.

A SECOND clobber path exists: `McpStatusRow.tsx:69` — removing a server chip on a NEW
chat calls `saveUserDefaults`, which PUTs `approval_mode: config?.approvalMode ||
'manual_approve'` (`McpComposer.store.ts:1040`). That writes a USER-DEFAULTS row,
poisoning branch B for every future conversation.

The default is currently spelled in five disagreeing places (`ApprovalMode` `#[default]`,
two `get_approval_mode` unwraps, `mcp.rs` branch C, `settings/repository.rs`
`DEFAULT_APPROVAL_MODE`), of which only four are diverged on `deploy-schedule`.

Hard constraints (user-confirmed):
- khoi behavior must be byte-identical (default stays `manual_approve`).
- An explicit user choice, or a per-tool auto-approve list, must still be persisted
  and honored — only the INITIAL default for a new, un-customized conversation changes.
- SQL: inline `COALESCE` inside the two EXISTING upsert queries only. **No new
  migration file. No seed SQL. No backfill SQL** (not in the repo, not in a PR body).
  Only the single row being upserted is ever touched — never a mass UPDATE/DELETE.
  Pre-existing rows are never mutated on any branch.
- Do not touch the built-in-server approval-bypass list or `usage_mode` handling.

## Items

- **ITEM-1**: Make `ApprovalMode::default()` the single source of truth for the
  deployment default. Document on the enum (`chat_extension/approval/models.rs`) that
  the `#[default]` variant is THE deployment default and the only intentional
  khoi↔deploy-schedule divergence.
- **ITEM-2**: Route `ConversationMcpSettings::get_approval_mode`
  (`approval/models.rs:119-123`) and `UserMcpDefaults::get_approval_mode`
  (`defaults/models.rs:102-106`) through `.unwrap_or_default()` instead of a hardcoded
  `ApprovalMode::ManualApprove`.
- **ITEM-3**: Route `settings/repository.rs`'s `DEFAULT_APPROVAL_MODE` through
  `ApprovalMode::default().to_string()` so `get_or_default` (`:104`) and the settings
  upsert (`:131`) — which also feed the project-scope GET at
  `project_extension/handlers.rs:42` — agree with the enum. Add a comment recording
  that the DB column default (`202607140180_mcp_schema.sql:56,132`) is unreachable
  because every INSERT in the tree supplies `approval_mode` explicitly, so no
  migration is needed.
- **ITEM-4**: Extract the three-branch approval resolution currently inline at
  `mcp/chat_extension/mcp.rs:2591-2604` into a pure
  `resolve_approval(settings, user_defaults) -> (ApprovalMode, Vec<AutoApprovedServer>)`
  and call it from `after_llm_call`. Branch C returns `ApprovalMode::default()`.
  Behavior-preserving; makes the precedence rule unit-testable.
- **ITEM-5**: Make `UpsertMcpSettingsRequest.approval_mode` (`approval/models.rs:218`)
  an `Option<ApprovalMode>` with `#[serde(default)]`, thread the `Option` through
  `approval/handlers.rs::update_mcp_settings`, `chat_extension/repository.rs:39` and
  `approval/repository.rs::upsert_conversation_settings`, and resolve it with an INLINE
  `COALESCE` in the EXISTING query: `COALESCE($3, '<default>')` on the VALUES arm and
  `COALESCE($3, mcp_settings.approval_mode)` on the `DO UPDATE` arm — mirroring the
  `auto_approved_tools` COALESCE already at `approval/repository.rs:82,86`. Absent
  field ⇒ never clobbers.
- **ITEM-6**: Same treatment for the user-defaults write path:
  `UpsertUserMcpDefaultsRequest.approval_mode` (`defaults/models.rs:168`) →
  `Option<ApprovalMode>` with `#[serde(default)]`, threaded through
  `defaults/handlers.rs::update_mcp_defaults` and
  `defaults/repository.rs::upsert_user_defaults` with the same inline COALESCE. This is
  what closes the `McpStatusRow` chip-removal clobber.
- **ITEM-7**: Add `default_approval_mode: ApprovalMode` to
  `UserMcpDefaultsGetResponse` (`defaults/handlers.rs:22-25`) so the client can learn
  the server default. `defaults` stays `Option` and stays `null` when unset (no
  synthesized row — that would fabricate id/user_id/timestamps and flip
  `McpInitializer.tsx:39`).
- **ITEM-8**: Regenerate the OpenAPI spec + TS client (`just openapi-regen`) for the
  ITEM-5/6/7 type changes; the `openapi::emit_ts::tests::types_ts_parity` golden test
  must stay green.
- **ITEM-9**: New pure frontend module
  `ui/src/modules/mcp/stores/approvalDefaults.ts` exporting `blankMcpConfig(serverDefault)`,
  `effectiveApprovalMode(configMode, serverDefault)` and the last-resort
  `FALLBACK_APPROVAL_MODE = 'manual_approve'` (used only before the defaults fetch
  resolves / on network failure). Mirrors the existing pure-module extraction
  `ui/src/modules/mcp/stores/approvalRouting.ts`.
- **ITEM-10**: `McpComposer.store.ts` — add `serverDefaultApprovalMode` state, set it
  from the `loadUserDefaults` response (`:1004-1008`), and replace the hardcoded
  `'manual_approve'` config-creation literals (`:321, 347, 525, 570, 597, 669, 693,
  743, 837`) with the ITEM-9 helper.
- **ITEM-11**: `McpComposer.store.ts` PUT sites — `saveConversationConfig` (`:409`),
  `saveProjectConfig` (`:485`) and `saveUserDefaults` (`:1040`) must OMIT
  `approval_mode` when the config has none, instead of substituting `'manual_approve'`.
  The `disabled_servers` server-list snapshot keeps being written (it is load-bearing);
  only the approval-mode pinning goes away.
- **ITEM-12**: `ui/src/modules/mcp/chat-extension/extension.tsx` — the
  settings-absent (`:1092-1098`) and error (`:1111-1117`) fallback configs use the
  server default instead of `'manual_approve' as const`.
- **ITEM-13**: `ui/src/modules/mcp/components/McpConfigModal.tsx:85` — the DISPLAY
  fallback `conversationConfig?.approvalMode || 'manual_approve'` is reached whenever
  the modal opens before a config exists for the active key (a brand-new chat opened
  before `McpInitializer` seeds). It must show the SERVER default, otherwise the
  approval radio (`:401`) tells the user "Manual" on a deployment that will in fact
  auto-approve. Route it through the ITEM-9 helper.
  (`ProjectMcpSettingsPanel.tsx:44`'s identical-looking fallback is NOT touched: the
  project GET uses `get_or_default` (`project_extension/handlers.rs:42`), so `settings`
  is never null there and the `||` arm is unreachable — it inherits the corrected
  default via ITEM-3.)

- **ITEM-14**: *(added in phase 5 — see DRIFT-1)* Fix the SIBLING tri-state field in
  the same two upserts. `auto_approved_tools`'s "None = preserve existing DB value"
  contract (`approval/repository.rs:66-70,86`, `defaults/repository.rs:46-50,66`) has
  never worked: the absent case bound `serde_json::Value::Null`, which encodes as the
  JSON value `null` — NOT SQL NULL — so `COALESCE($4, <table>.auto_approved_tools)`
  took the first arm and OVERWROTE the stored list with JSON null (read back as `[]`
  via `unwrap_or_default`). Any save that omitted the field therefore destroyed the
  user's per-tool allow-list. Bind `Option<serde_json::Value>` instead so `None` is a
  real SQL NULL. Same statement, same single row, no migration — and required by the
  task's hard constraint that a user who auto-approves specific tools must still have
  that persisted and honored.

## Files to touch

Backend (`src-app/server/src/modules/mcp/`):
- `chat_extension/approval/models.rs` (ITEM-1, 2, 5)
- `chat_extension/approval/repository.rs` (ITEM-5)
- `chat_extension/approval/handlers.rs` (ITEM-5)
- `chat_extension/repository.rs` (ITEM-5 signature passthrough)
- `chat_extension/defaults/models.rs` (ITEM-2, 6)
- `chat_extension/defaults/repository.rs` (ITEM-6)
- `chat_extension/defaults/handlers.rs` (ITEM-6, 7)
- `chat_extension/mcp.rs` (ITEM-4)
- `settings/repository.rs` (ITEM-3)
- `project_extension/handlers.rs` (ITEM-5 passthrough only if the shared signature moves)

Backend tests:
- `src-app/server/tests/mcp/conversation_settings_default_test.rs` (new)
- `src-app/server/tests/mcp/mcp_defaults_test.rs` (extend)
- `src-app/server/tests/mcp/mod.rs` (register the new test module)

Generated (ITEM-8, excluded from the audit coverage law):
- `src-app/ui/openapi/openapi.json`, `src-app/ui/src/api-client/types.ts`
- `src-app/desktop/ui/openapi/openapi.json`, `src-app/desktop/ui/src/api-client/types.ts`

Frontend (`src-app/ui/src/modules/mcp/`):
- `stores/approvalDefaults.ts` (new, pure — ITEM-9)
- `stores/approvalDefaults.test.ts` (new)
- `stores/McpComposer.store.ts` (ITEM-10, 11)
- `chat-extension/extension.tsx` (ITEM-12)
- `components/McpConfigModal.tsx` (ITEM-13)

Frontend tests:
- `src-app/ui/tests/e2e/chat/mcp-approval-default-persistence.spec.ts` (new)

Gallery fixture (added in phase 5 — see DRIFT-1.2; a newly-required response field
broke the recorded cassette's `tsc` check):
- `src-app/ui/src/dev/gallery/fixtures/recorded/crawl.json`
- `src-app/ui/scripts/gen-crawl-cassette.mjs` (+ its regenerated `crawl.generated.ts`)

## Patterns to follow

- **Inline COALESCE upsert (ITEM-5, 6)** — mirror the `auto_approved_tools` handling
  ALREADY in `chat_extension/approval/repository.rs:66-70,82,86`: an `Option` param,
  `serde_json::Value::Null` / a bound `Option<String>` for "absent", and
  `COALESCE($n, <table>.<col>)` on the conflict arm. Same shape, same query, no new
  statement.
- **Pure extraction for testability (ITEM-4)** — mirror
  `chat/extensions/project/project.rs::apply_project_context` (a pure function pulled
  out of an extension hook purely so it is unit-testable) and
  `mcp/tool_calls/record.rs`'s in-source `#[cfg(test)]` block.
- **Pure frontend module (ITEM-9)** — mirror
  `ui/src/modules/mcp/stores/approvalRouting.ts` (enum-free, importable without the
  store) and its `node:test` companion `ui/src/modules/mcp/chat-extension/toolRun.test.ts`.
- **Additive response field (ITEM-7)** — mirror the existing
  `UserMcpDefaultsGetResponse`/`McpSettingsResponse` shape in
  `chat_extension/{defaults,approval}/handlers.rs`; keep the `Option` field's meaning
  unchanged and add a sibling.
- **Integration test style (tests)** — mirror
  `src-app/server/tests/mcp/mcp_defaults_test.rs` (bare `reqwest` helpers +
  `test_helpers::create_user_with_permissions`, `serde_json::json!` payloads,
  `TestServer::start()`).

## JTBD / UX design

No new UI surface, no new page, drawer, card, list, or permission. The user-visible
behavior change is exactly one thing: **on a deployment configured to auto-approve MCP
tools, a new conversation keeps auto-approving on every turn instead of only the
first.** There is nothing new to click.

The only rendered difference is the MCP config modal's approval-mode radio
(`McpConfigModal.tsx:401`) for a BRAND-NEW chat on such a deployment: it now shows the
server's actual default (Auto) instead of falsely showing Manual. That is a
correctness fix to an existing control, not a new surface — no precedent/scale/
responsive/populated-render work applies. Existing modal states already have gallery
coverage (`ui/src/modules/mcp/gallery.tsx`).

Entity-lifecycle: the only entity is the per-conversation `mcp_settings` row. It is
created by the existing turn-1 auto-persist, updated by the existing modal save,
and cascade-deleted with the conversation. This change adds no new entity and no new
lifecycle edge.

## Risks / non-goals

- NOT touching `is_builtin_server_id` / `auto_attach_builtin_ids` (built-in bypass) or
  `usage_mode` — explicitly out of scope.
- NOT touching the DB column defaults and NOT adding a migration (ITEM-3 documents why
  the column default is unreachable).
- NOT backfilling already-clobbered rows on the live instance (user decision: code fix
  only).
- The project-scope PUT (`project_extension/handlers.rs:114`) always sends
  `Some(approval_mode)` today and is only ever driven by an explicit user action in the
  project modal, so it is unaffected; it inherits the ITEM-3 default via
  `get_or_default` on read.
