# TESTS — fix-mcp-auto-approve-default

Every ITEM-1..13 is covered by ≥1 test below. No item is `[DESCOPED]`.

**Branch-agnostic assertion discipline.** The same test files must pass on BOTH
`fix/mcp-auto-approve-default` (khoi, default `manual_approve`) and
`fix/mcp-auto-approve-default-deploy` (deploy-schedule, default `auto_approve`). So no
test hardcodes `"manual_approve"` as the EXPECTED default — Rust tests assert against
the compiled `ApprovalMode::default()`, and the integration/e2e tests assert the
persisted value **equals whatever `GET /api/mcp/defaults`.`default_approval_mode`
reports**. Tests that assert an EXPLICIT user choice is honored do use literals, since
that value is supplied by the test itself and is branch-independent.

**No new permission** is introduced (no `modules/*/permissions.rs` change, no migration
grant), so A10's `[negative-perm]` restricted-user e2e does not apply. The endpoints
touched keep their existing gates (`conversations::read` / `conversations::edit`),
whose 403 paths are already covered by `mcp_defaults_test.rs:104,297` — TEST-16 pins
that they still 403 after the request-type change.

## Unit (Rust)

- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/approval/models.rs` — asserts: `ConversationMcpSettings::get_approval_mode()` returns the parsed value for each of the three valid strings, and returns exactly `ApprovalMode::default()` (not a hardcoded `ManualApprove`) for an unparseable stored string — so the branch's compiled default is the single fallback.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/server/src/modules/mcp/chat_extension/defaults/models.rs` — asserts: `UserMcpDefaults::get_approval_mode()` has the same parse/fallback behaviour, falling back to `ApprovalMode::default()`.
- **TEST-3** (tier: unit) [covers: ITEM-1] file: `src-app/server/src/modules/mcp/chat_extension/approval/models.rs` — asserts: `ApprovalMode::default().to_string()` round-trips through `FromStr` back to `ApprovalMode::default()`, so the enum default and its DB string spelling can never drift apart (this is what ITEM-3's `to_string()` derivation relies on).
- **TEST-4** (tier: unit) [covers: ITEM-3] file: `src-app/server/src/modules/mcp/settings/repository.rs` — asserts: the module's default-approval-mode value equals `ApprovalMode::default().to_string()` — i.e. the settings repository's no-row default and the enum agree, closing the disagreement this bug was built on.
- **TEST-5** (tier: unit) [covers: ITEM-4] file: `src-app/server/src/modules/mcp/chat_extension/mcp.rs` — asserts: `resolve_approval` precedence — (a) conversation settings present ⇒ its mode + its auto-approved list win even when user defaults disagree; (b) no conversation settings + user defaults present ⇒ the defaults' mode + list; (c) neither ⇒ `(ApprovalMode::default(), [])`. Case (c) is the exact branch the live bug fell through on turn 1.
- **TEST-6** (tier: unit) [covers: ITEM-5] file: `src-app/server/src/modules/mcp/chat_extension/approval/models.rs` — asserts: `UpsertMcpSettingsRequest` deserializes with `approval_mode` ABSENT into `None` (not `Some(ApprovalMode::default())`), and with it present into `Some(mode)` — the "absent is distinguishable" property the COALESCE depends on.
- **TEST-7** (tier: unit) [covers: ITEM-6] file: `src-app/server/src/modules/mcp/chat_extension/defaults/models.rs` — asserts: the same absent-vs-present deserialization property for `UpsertUserMcpDefaultsRequest`.

## Unit (frontend, node:test)

- **TEST-8** (tier: unit) [covers: ITEM-9, ITEM-10] file: `src-app/ui/src/modules/mcp/stores/approvalDefaults.test.ts` — asserts: `blankMcpConfig(serverDefault)` stamps the SUPPLIED server default onto `approvalMode` (checked for all three modes, so a `'manual_approve'` literal could not pass), returns empty `selectedServers`/`disabledServers`/`autoApprovedTools`, and returns a FRESH `Map` per call (no shared mutable state across configs).
- **TEST-9** (tier: unit) [covers: ITEM-9, ITEM-13] file: `src-app/ui/src/modules/mcp/stores/approvalDefaults.test.ts` — asserts: `effectiveApprovalMode(configMode, serverDefault)` returns `configMode` when set (including when it differs from the server default — an explicit user choice must never be overridden) and `serverDefault` when `undefined`/absent; and that `FALLBACK_APPROVAL_MODE` is only used when the server default itself is unknown.
- **TEST-10** (tier: unit) [covers: ITEM-11] file: `src-app/ui/src/modules/mcp/stores/approvalDefaults.test.ts` — asserts: the payload builder used by the PUT sites OMITS the `approval_mode` key entirely (`!('approval_mode' in payload)`) when no mode is set, and includes it when one is — so an un-customized save can never clobber the stored/server default.

## Integration (Rust)

- **TEST-11** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/mcp/conversation_settings_default_test.rs` — asserts: PUT `/conversations/{id}/mcp-settings` with `approval_mode` OMITTED on a conversation that has NO row creates the row with the server default, and a follow-up GET returns that same mode. Expected value is read from `GET /api/mcp/defaults`.`default_approval_mode`, so it proves the fix on deploy and the no-regression on khoi with one assertion.
- **TEST-12** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/mcp/conversation_settings_default_test.rs` — asserts: **pre-existing rows are never mutated.** PUT an EXPLICIT `manual_approve`, then PUT again with `approval_mode` omitted → GET still returns `manual_approve`. Repeated with an explicit `auto_approve` → still `auto_approve`. This is the direct test of the user-confirmed constraint that an omitted field touches nothing.
- **TEST-13** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/mcp/conversation_settings_default_test.rs` — asserts: an explicit user choice still round-trips (PUT `disabled`/`manual_approve`/`auto_approve` each persist and are returned verbatim), and that a PUT omitting `approval_mode` still writes `disabled_servers` — the server-list snapshot the turn-1 auto-persist exists for must not be lost.
- **TEST-14** (tier: integration) [covers: ITEM-5] file: `src-app/server/tests/mcp/conversation_settings_default_test.rs` — asserts: a per-tool `auto_approved_tools` allow-list set under `manual_approve` survives a later PUT that omits both `approval_mode` and `auto_approved_tools` — the two COALESCE arms compose and neither clobbers the other.
- **TEST-15** (tier: integration) [covers: ITEM-6] file: `src-app/server/tests/mcp/mcp_defaults_test.rs` — asserts: PUT `/mcp/defaults` with `approval_mode` OMITTED creates the row with the server default; a later omitted-field PUT preserves an explicitly-set mode. This is the fix for the `McpStatusRow` chip-removal clobber that poisons every future conversation.
- **TEST-16** (tier: integration) [covers: ITEM-6, ITEM-7] file: `src-app/server/tests/mcp/mcp_defaults_test.rs` — asserts: the existing gates and shape are intact after the request/response type change — GET still 403s without `conversations::read`, PUT still 403s without `conversations::edit`, and GET still returns `defaults: null` for a user with no row (no synthesized row).
- **TEST-17** (tier: integration) [covers: ITEM-7, ITEM-8] file: `src-app/server/tests/mcp/mcp_defaults_test.rs` — asserts: `GET /api/mcp/defaults` returns a `default_approval_mode` field that is one of the three valid modes AND is the value a defaults-less conversation actually gets persisted (cross-checked against TEST-11's flow), proving the client is told the truth.
- **TEST-18** (tier: integration) [covers: ITEM-4] file: `src-app/server/tests/mcp/mcp_extension_test.rs` — asserts: the existing MCP chat-extension behaviour is unchanged by the `resolve_approval` extraction (re-run as the no-regression guard for a pure refactor).
- **TEST-19** (tier: integration) [covers: ITEM-8] file: `src-app/server/src/openapi/emit_ts.rs` — asserts: `openapi::emit_ts::tests::types_ts_parity` — the committed `types.ts` is byte-identical to what the committed `openapi.json` generates, i.e. the regen for the ITEM-5/6/7 type changes was actually run and committed.

## E2E (Playwright)

- **TEST-20** (tier: e2e) [covers: ITEM-10, ITEM-11, ITEM-12] file: `src-app/ui/tests/e2e/chat/mcp-approval-default-persistence.spec.ts` — asserts: **the literal reported repro.** On a fresh chat with no user-defaults row, send a first message so the conversation is minted and the frontend's turn-1 auto-persist fires; then `GET /api/conversations/{id}/mcp-settings` and assert `approval_mode` EQUALS `GET /api/mcp/defaults`.`default_approval_mode`. Today this write pins `manual_approve` regardless of the server default — this is the assertion that fails before the fix on deploy.
- **TEST-21** (tier: e2e) [covers: ITEM-6, ITEM-11] file: `src-app/ui/tests/e2e/chat/mcp-approval-default-persistence.spec.ts` — asserts: the SECOND clobber path. With no user-defaults row, remove an MCP server chip on the new-chat page (the real `McpStatusRow` close button, per `mcp-chip-row-persistence.spec.ts`); the resulting `GET /api/mcp/defaults` row must record the removal in `disabled_servers` AND carry `approval_mode` equal to `default_approval_mode` — proving the chip removal no longer writes a hardcoded `manual_approve` that poisons every future conversation.
- **TEST-22** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/chat/mcp-approval-default-persistence.spec.ts` — asserts: the MCP config modal opened on a brand-new chat DISPLAYS the approval mode matching `default_approval_mode` (real DOM: the checked radio in `mcp-config-modal`), so the user is not told "Manual" on a deployment that auto-approves. Rendering-truth check per B7 — a unit test on the store cannot prove what the radio shows.
