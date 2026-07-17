# PLAN_AUDIT — agent-core (audited against the SDK base @ 46f605dc5)

A fresh whole-plan audit against the post-SDK-extraction codebase. **No `BLOCKED`.** The SDK carve left
every host seam (chat/workflow/mcp) app-side and intact; the platform layer (AppError/permissions/sync/
migrations) is now SDK-sourced but the plan's references hold with the corrections below (already folded
into PLAN.md/BASE.md).

## Breakage risk
- **Chat migration (ITEM-24..26) is the highest risk**, unchanged by the SDK: `streaming.rs` loop +
  `DeltaAccumulator` (:1241) + `ChatExtension` (registry.rs:97, full hook surface incl. pause/resume
  `should_create_user_message`/`provide_assistant_message`/`after_user_message_created`) + the
  chat-owned `SSEChatStreamEvent` (`compose_chat_stream_events` macro). Behaviour-preserving; guarded by
  TEST-24 golden + TEST-38/39 (existing chat suites unchanged). SDK note: `ExtensionEntry`/`Registry` are
  now newtypes over `ziee_framework::entity_extension` — ITEM-32 mirrors that, not a hand-roll.
- **`ToolDispatcher → call_mcp_tool` extraction (ITEM-21)**: the MCP path is INLINED in
  `ToolDispatcher::dispatch` (`dispatch.rs:1197-1345`) today — no helper exists, so the extraction is a
  clean new function; both callers pass `enforce_conversation_disabled` (DEC-17). Symbol-preserving.
- **`startup_sweep`/`resumable_agent` (ITEM-17)**: `fail_orphaned_runs` fails only pending/running and
  spares `waiting` (`repository.rs:823`); gate the new spare-behaviour to agent runs via the new
  `resumable_agent` column. Additive.
- Everything else is additive/greenfield (`src-app/agent-core`, `modules/agent/`, `agent_dispatch.rs`,
  the new `StepConfig::Agent` arm) — no existing caller removed.

## Pattern conformance
- **Error/perm/sync now SDK-sourced (verified):** `crate::common::{ApiResult,AppError}` re-export
  `ziee_core` (`common/type.rs:17`); permissions are `ziee_identity::PermissionCheck` with **four** assoc
  consts (`NAME/PERMISSION/DESCRIPTION/MODULE`, `permission.rs:14-23`) — ITEM-28 corrected; sync is the
  app-side `modules/sync/event.rs` `SyncEntity` enum impl'ing `ziee_framework::SyncEntityKind`/`SyncSurface`
  — a new variant + `publish(...)` call, as before.
- **Settings mirror (ITEM-28/30):** `code_sandbox_settings` is a clean end-to-end template (module
  migration singleton + repo/cache + REST + `PermissionCheck` admin-only-no-grant + `SyncEntity` variant
  + `settingsAdminPages` card). Copy a *server-side-only* singleton sibling (`MemoryAdminSettings`/
  `SessionSettings`) — NOT the `ziee-sandbox` model/cache split (engine-forced, not a settings convention).
- **New step kind (ITEM-18):** `StepConfig` in `validate.rs:150`, `StepKindTag` in `types.rs:151`,
  dispatcher `match` in `runner.rs:736-749`; all exhaustive arms present. Workflow imports no SDK crate
  directly → `AgentDispatcher` fits without an SDK-import refactor.
- **Extension seam (ITEM-32):** mirror `ziee_framework::entity_extension::{ExtensionEntry,ExtensionRegistry,
  sorted_entries}` (the chat registry already does, `registry.rs:32,337`).

## Migration collisions
- Module-owned (N7). Highest counter = SDK `ziee-seed` `202607150000`; app modules top out at
  `202607146095`. This branch's `20260716NNNN` band sorts above both → **no collision**. merge-gate C2
  checks the union of module ∪ SDK-crate dirs. New migrations: `modules/agent/migrations/…agent_admin_settings`,
  `modules/workflow/migrations/…workflow_agent_step` (agent_transcript_json + resumable_agent + status
  CHECK), `mcp` module's `review_classification` column. No grant migration (`agent::settings::*` = admin `*`).

## OpenAPI regen
- **Required.** New `GET/PUT /api/agent/settings` + `AgentAdminSettings` types + the new app-side
  `SyncEntity` variant ⇒ `just openapi-regen` BOTH `ui/` + `desktop/ui/` (generated files excluded from
  coverage-law + FE gates).

---

## SDK-base findings folded in (from the audit)
- **F1 (ITEM-1 wording):** deps are direct per-crate path deps, not `[workspace.dependencies]`. Fixed.
- **F2 (DEC):** `ziee-core` transitively pulls axum+sqlx into the crate; "no sqlx" = no `query!` macros.
  → DEC in Phase 4.
- **F3 (ITEM-28):** `PermissionCheck` needs the 4th const `DESCRIPTION`. Fixed.
- **F4 (BASE):** migration max is `202607150000` (SDK ziee-seed); union-checked. Fixed.
- **F5 (§7.2/ITEM-20):** control_mcp has no in-process `ToolProvider` — it's a built-in MCP server
  (`control.ziee.internal`, loopback HTTP); `McpToolProvider` surfaces it via the allow-list. Fixed.
- **F6 (DEC + TEST, ITEM-2/20):** `ModelResolver` sources = `create_provider_from_model_id`
  (`chat/core/ai_provider/mod.rs:43`, global-`Repos`-coupled) + `user_has_access_to_provider` /
  `validate_model_access` RBAC. The workflow→chat import is precedented (`runner.rs:1277,1385`). → DEC to
  accept/relocate + a TEST asserting `resolve` DENIES an inaccessible model.

## Per-item verdicts
- **ITEM-1** — verdict: CONCERN — crate feasible + clean DAG (`ai-providers`+`ziee-core`+`ziee-identity`, no cycle); direct path deps (F1); `ziee-core` pulls axum+sqlx transitively (F2 → DEC).
- **ITEM-2** — verdict: PASS — six port traits; `ModelResolver` building blocks exist (F6); `#[async_trait]` genericity mirrors `IdentityResolver`.
- **ITEM-3** — verdict: PASS — crate-local `ToolResult` justified (`ContentBlock::ToolResult` lacks `structured_content`, `ai-providers/chat.rs:150`); driver→`ziee_core::AppError`.
- **ITEM-4** — verdict: PASS — loop mirrors `run_llm_call` + `DeltaAccumulator`; provider streaming-first.
- **ITEM-5** — verdict: PASS — caps mirror `PER_STEP_TOKEN_CAP`/`SAFETY_MAX_ITERATIONS`.
- **ITEM-6** — verdict: PASS — core `CompactionExtension`; reuse `summarizer.rs`+`conversation_summaries` behind `replace_head`; crate-local `estimate_tokens`.
- **ITEM-7** — verdict: PASS — `fan_out` semaphore mirrors `llm_map`; per-child model via `ModelResolver`.
- **ITEM-8** — verdict: PASS — core-injected `update_plan` tool.
- **ITEM-9** — verdict: PASS — app-side verification extension (crate stays domain-free).
- **ITEM-10** — verdict: CONCERN — `concise|detailed` additive default; don't break `structured_content` consumers.
- **ITEM-11** — verdict: PASS — matrix in `ApprovalPolicy::decide`; bwrap SandboxMode enforcement descoped (DEC-2).
- **ITEM-12** — verdict: CONCERN — reviewer model via `ModelResolver` (F6); classification → new `mcp_tool_calls.review_classification` (ITEM-22).
- **ITEM-13** — verdict: CONCERN — escalation logic; `ApprovedForSession` allow-rule persistence app-side.
- **ITEM-14** — verdict: PASS — `mcp_tool_calls` has `workflow_run_id` (`mcp` migration); journal reuse.
- **ITEM-15** — verdict: PASS — durable gate mirrors `ElicitDispatcher` (`dispatch.rs:1448`).
- **ITEM-16** — verdict: PASS — resume = reload transcript + continue; idempotency key.
- **ITEM-17** — verdict: CONCERN — `resumable_agent` column gates the sweep-spare to agent runs (`startup_sweep`/`fail_orphaned_runs` bulk UPDATE).
- **ITEM-18** — verdict: PASS — all exhaustive arms verified (`validate.rs:150/223`, `types.rs:151`, `runner.rs:736`); `invocation_source` already permits `'agent'`.
- **ITEM-19** — verdict: PASS — mirrors `LlmDispatcher`/`ToolDispatcher`.
- **ITEM-20** — verdict: CONCERN — port impls; `McpToolProvider` surfaces `control.ziee.internal` via allow-list (F5); `WorkflowModelResolver` (F6).
- **ITEM-21** — verdict: CONCERN — clean extraction (path is inlined today, `dispatch.rs:1197`); parity test required (TEST-16); `enforce_conversation_disabled` param (DEC-17).
- **ITEM-22** — verdict: PASS — additive module-owned migrations; `models.rs`/`repository.rs` accessors.
- **ITEM-23** — verdict: PASS — `cost.rs` Agent arms like `llm_map`.
- **ITEM-24** — verdict: CONCERN (highest risk) — chat host adapter; behaviour-preserving; pause/resume orchestration lives here; e2e-guarded.
- **ITEM-25** — verdict: CONCERN — re-expression not lift; mirror `entity_extension`; preserve ordering + SSE.
- **ITEM-26** — verdict: CONCERN — SSE parity incl. raw ext events (`compose_chat_stream_events`); `DeltaAccumulator`-equiv persistence.
- **ITEM-27** — verdict: PASS — `delegate` core-injected (no A8).
- **ITEM-28** — verdict: PASS — mirror `code_sandbox`/`MemoryAdminSettings`; 4-const `PermissionCheck` (F3); server-side model+repo+cache; admin `*`, no grant.
- **ITEM-29** — verdict: PASS — UI mirrors workflow builder + run-view SSE; needs `tier: e2e`.
- **ITEM-30** — verdict: PASS — settings card mirrors `code_sandbox` `settingsAdminPages`; gated `agent::settings::read`.
- **ITEM-31** — verdict: PASS — content-type renderer + progress stream; needs `tier: e2e`.
- **ITEM-32** — verdict: PASS — server-owns `AGENT_EXTENSIONS` mirroring `entity_extension`; crate = trait only (no linkme).

## Decisions raised (carried to Phase 4)
- **DEC-A (F2):** accept `ziee-core`-transitive axum+sqlx in `agent-core` (cost of the `AppError` return type).
- **DEC-B (F6):** `ModelResolver` source location — accept the workflow→chat `create_provider_from_model_id` import (precedented) vs relocate to a shared module. + a TEST asserting RBAC deny.
- Plus the settled product decisions carried forward from the prior session (unattended policy / reviewer default / sandbox scope) — see DECISIONS.md.
