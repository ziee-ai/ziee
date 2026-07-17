# PLAN — agent-core (the full agent architecture, ONE integrated implementation, on the SDK base)

Build the shared **`AgentCore`** (loop + compaction + subagents behind six ports) as a **ziee-app crate
`src-app/agent-core`** (NOT an SDK crate — it's ziee-only; `ai-providers` stays app-side), and wire it
into **all three hosts in one branch** — chat (replaces the current chat loop), the workflow
`kind: agent` step, and parallel fan-out — plus the "friendlier-than-Claude-Code" layer (durable review
gate, reviewer agent, visible progress, admin policy, grounded-answer verification), and the SDK
cross-app dimension (drive companion apps via `control_mcp`). One cohesive system, validated integrated
at Phase 8.

**Base:** `origin/main @ 46f605dc5` (post-SDK-extraction). **Design authority:**
`/data/pbya/ziee/tmp/agent-arch-wt/ZIEE_AGENT_ARCHITECTURE.md` (replanned 2026-07-16 — §0.5 SDK
re-framing; a copy is committed here as `.lifecycle/agent-core/DESIGN_REFERENCE.md`).

## SDK-base deltas from the prior (pre-SDK) plan — read first
- **Crate is `src-app/agent-core` in the ZIEE workspace** (not `sdk/crates/*`); deps `ai-providers`
  (app-side, unchanged) + **`ziee-core`** (`AppError`/`ApiResult`/macros) + **`ziee-identity`**
  (`Principal`/`PermissionCheck`). **No N9** (may name ziee domain). **No `ai-providers` relocation.**
- **Driver returns `ziee_core::AppError`** (app-wide now); pure ports keep associated `Error` types
  (à la `ziee_identity::TokenVerifier::Error`). Supersedes the bespoke `AgentError`.
- **Migrations are MODULE-OWNED (SDK N7):** `modules/<m>/migrations/<YYYYMMDDNNNN>_<m>_<desc>.sql`,
  build.rs globs them — NOT a flat numbered dir. The agent settings migration lives in the new app
  `agent` module; the workflow-run columns in `modules/workflow/migrations/`.
- **Permissions** are the `ziee_identity::PermissionCheck` trait; **sync** is the
  `ziee_framework::SyncEntityKind`/`SyncSurface` seam (app-side `SyncEntity`).
- **Cross-app driving:** `control_mcp` is a **built-in MCP server** (`control.ziee.internal`, loopback
  `/api/control/mcp`, SDK dispatch core `ziee-control-mcp` + app `control_mcp` module). The agent reaches
  it like any built-in — `McpToolProvider` (ITEM-20) surfaces `control.ziee.internal` in its allow-list;
  **no bespoke cross-app ToolProvider impl.** (§7.2 of the design.)

## Build order (internal to Phase 5)
1. **Crate scaffold + core + ports + fakes** (ITEM-1..10). 2. **Workflow host** (18..23). 3. **Safety +
durability** (11..17). 4. **Chat host** (24..26). 5. **Fan-out surface + config/UI** (27..32).

## Items

<!-- === A. The shared core + ports (the ziee-app crate) === -->
- **ITEM-1**: New **`src-app/agent-core` crate** (ziee workspace member; deps `ai-providers` + `ziee-core` + `ziee-identity`; build-DB-free, no `sqlx`). PLUS a thin app-side `server/src/modules/agent/` for the host-coupled surface (settings/routes/permissions + registration). Add `agent-core` to `src-app/Cargo.toml` `[workspace] members`; declare `ai-providers` (`../server/ai-providers`) + `ziee-core` + `ziee-identity` (`../../sdk/crates/…`) as **direct per-crate path deps** (mirror `server/Cargo.toml`), NOT `[workspace.dependencies]` hoisting (the in-tree pattern is direct path deps). NB: depending on `ziee-core` for `AppError` transitively pulls axum+sqlx into the crate — "build-DB-free / no sqlx" means no `query!` macros (no build DB), not a leaf tree (DEC).
- **ITEM-2**: **Six port traits** in `agent-core/src/ports.rs` — `TranscriptStore`, `EventSink`, `ToolProvider`, `HumanGate`, `ApprovalPolicy`, `ModelResolver` (`async resolve(model_id, user_id) -> Result<Arc<Provider>>` — the seam that mints a per-child/reviewer provider without the crate touching the DB/RBAC; the app impl composes the model-access check + provider build). All `#[async_trait] Send+Sync`; `AgentCore` generic over injected `Arc<P>` (the `ziee-identity`/`IdentityResolver` pattern).
- **ITEM-3**: Core types in `agent-core/src/types.rs` — `AgentEvent {Message,Usage,ToolNotification,HistoryReplaced,GateOpened,Stopped}`, `AgentTurnRequest`, `TurnSeed`, `StopReason`, `ReviewDecision {Approved,ApprovedForSession,Denied,Abort}`, `GateAsk/GateOutcome/GateTicket`, `ToolCallRecord`, `IdempotencyKey`, `Budget`, `Decision {Auto,Prompt,Review,Deny}`, `SandboxMode`, `ApprovalMode`, `SubagentSpec/SubagentSummary/SubagentLimits`, and a crate-local **`ToolResult { content: Vec<ai_providers::ContentBlock>, is_error: bool, structured_content: Option<serde_json::Value> }`** (ai-providers `ContentBlock::ToolResult` lacks `structured_content`). Errors: driver → `ziee_core::AppError`; pure ports → associated `Error`.
- **ITEM-4**: `AgentCore` + `AgentCore::run(req, cancel) -> impl Stream<AgentEvent>` (`agent-core/src/core.rs`): rebuild context → run the `AgentExtension` pipeline (incl. the core `CompactionExtension`) → `provider.chat_stream` **with tools** → cancel-aware drain of text + `ToolUseDelta` → extract `ToolUse` → per-tool approval-gate → `ToolProvider::call` → journal + append → loop. Tool requests ride inside `Message` blocks.
- **ITEM-5**: Stop conditions + `Budget` — `NoToolCall`, `IterationCap` (`max_steps`), `TokenCap` (per-run + per-step), `WallClock`/`Halted` (cancel). On `IterationCap`, synthesize `is_error` results for unexecuted `ToolUse`s (no orphan).
- **ITEM-6**: **Compaction (C1) — a CORE (always-on) `AgentExtension`.** `Compactor::fit` (`agent-core/src/compaction.rs`): sliding window (summarize oldest ~30% via a `Provider` call, keep ~70%, escalate ~10% on overflow); "pinned" (core-memory) blocks kept verbatim; summary persisted via `TranscriptStore::replace_head`; evicted raw retained; emits `HistoryReplaced`; a crate-local `estimate_tokens` (copied from the ziee `common/tokens.rs`, ~chars/4). Invoked via a `before_model` hook at late `order` — the loop has NO bespoke `compactor.fit()`.
- **ITEM-7**: **Subagent fan-out (C2)** — `AgentCore::fan_out(children, cancel) -> Vec<SubagentSummary>`: fresh child per spec (own budget), bounded `SubagentLimits {max_depth=1, max_threads=6}` (semaphore), summaries-not-transcripts. Per-child `model_id` resolved via the injected **`ModelResolver`** (RBAC-bound); child `ToolProvider::list` omits `delegate` (enforces `max_depth=1`).
- **ITEM-8**: **Plan/todo tool (C3)** — a **core-injected** `update_plan` tool (NOT an MCP server); the todo list streams as an `AgentEvent`/SSE so chat + run views render an evolving plan.
- **ITEM-9**: **Verification/ground-truth (C4)** — the app supplies `citations`/`knowledge_base` as tools + a grounding system nudge (app-side `AgentExtension`; the crate stays domain-free).
- **ITEM-10**: **Tool-ACI `concise|detailed` convention (C5)** on built-in tool results (concise default), threaded through `structured_content`.

<!-- === B. Safety === -->
- **ITEM-11**: **Approval matrix (P8)** — `SandboxMode × ApprovalMode` (Codex enums) compose in `ApprovalPolicy::decide -> Decision`. `TrustedAutoApprovePolicy` auto-approves read-only/trusted built-ins (`ToolProvider::is_trusted`). (Technical per-call bwrap SandboxMode enforcement is descoped — DEC-2; ship the policy/approval half.)
- **ITEM-12**: **Reviewer agent (F2, auto_review)** — before a `Prompt` escalates to a human, a cheap fan-out child (model resolved from `reviewer_model_id` via `ModelResolver`) risk-classifies exfiltration/credential-probe/destructive/persistence → `Low→Auto`, `High→Prompt(gate)`, `Critical→Deny`; **fail-closed**; runs ONLY for approval-needing calls; classification stored on `mcp_tool_calls`; steered by the admin `reviewer_policy` text.
- **ITEM-13**: **Escalation (with_escalated_permissions)** — a mutating/external op emits a durable gate carrying the proposed widened perms; approve → an `ApprovedForSession` per-conversation allow-rule (no silent escalation).

<!-- === C. Durability === -->
- **ITEM-14**: **Journal at the completed-tool-call boundary (P5)** — `TranscriptStore::journal_tool_call`/`completed_tool_calls` reuse `mcp_tool_calls`; one row per completed tool call.
- **ITEM-15**: **Durable human gate (F1)** — `HumanGate::request` returns `GateOutcome::Suspended` (workflow: `persist_pending` + `mark_status(Waiting)`; chat: live pause). Mirrors `ElicitDispatcher` + the chat approval path.
- **ITEM-16**: **Resume replay + idempotency (P6)** — on gate-resume, reload the persisted transcript (completed tool_results already in it) + continue; only an in-flight un-journaled call re-runs, guarded by an idempotency key `<run_id>:<turn>:<ordinal>` threaded into the MCP call context.
- **ITEM-17**: **Crash-mid-loop resume (F6, workflow host)** — `startup_sweep` marks a crashed `running` agent run **`resumable`** (spared, dir kept) via a `workflow_runs.resumable_agent BOOLEAN` set while the runner is in an agent step; `resume_run` replays via ITEM-16. Chat opts OUT.

<!-- === D. Host 2 — workflow kind:agent === -->
- **ITEM-18**: `StepConfig::Agent { prompt, prompt_file, system, servers, max_steps, output_format, sandbox_mode?, approval_mode? }` in `validate.rs`; `StepKindTag::Agent`; relax `WORKFLOW_DEAD_TOOLS_FIELD` for the agent kind; every exhaustive-match arm (validate kind_str + collect-templates + tool-server validate; types StepKindTag; cost estimate/dry_run; compiled; type_infer; ref_check; runner require_model + dispatcher).
- **ITEM-19**: `AgentDispatcher: StepDispatcher` (`workflow/agent_dispatch.rs`) — builds `AgentCore` with workflow ports, runs `run`, folds tokens into `ctx.total_tokens`, honors `PER_STEP_TOKEN_CAP`, writes final output via `file_io::write_step_output(StepKindTag::Agent)`, returns `StepResult::{Completed,Suspended,Failed,Cancelled}`.
- **ITEM-20**: Workflow port impls — `McpToolProvider` (via shared `call_mcp_tool`, `list()` = server allow-list → `Vec<Tool>`, `is_trusted` = `is_builtin_server_id`), `WorkflowEventSink` (→ `SSEWorkflowRunEvent` StepProgress tracks), `WorkflowTranscriptStore` (`agent_transcript_json` + `mcp_tool_calls`), `WorkflowHumanGate` (durable elicit), a `WorkflowModelResolver` (app model-resolve + RBAC).
- **ITEM-21**: Refactor the shared MCP tool-call path out of `ToolDispatcher::dispatch` into a reusable `call_mcp_tool(..., enforce_conversation_disabled: bool)` so `ToolDispatcher` (passes `true`, as today) AND `McpToolProvider` share ONE impl; **chat passes `false` to preserve its current disabled-server behavior** (DEC-17 — the refactor flips NEITHER path).
- **ITEM-22**: Module-owned migrations (N7): `modules/workflow/migrations/<ts>_workflow_agent_step.sql` (`workflow_runs.agent_transcript_json JSONB` + `resumable_agent BOOLEAN` + status-CHECK `resumable`); `mcp_tool_calls.review_classification` column (in the owning module's migrations). `models.rs`/`repository.rs` accessors.
- **ITEM-23**: `cost.rs` estimate/dry_run `Agent` arms (runtime-dependent like `llm_map`; `est_calls ≤ max_steps`); `require_model` = true.

<!-- === E. Host 1 — chat === -->
- **ITEM-24**: Chat host adapter — `chat/core/services/streaming.rs` constructs `AgentCore` with chat ports (`ChatTranscriptStore` = chat repos + `conversation_summaries`; `ChatEventSink` → `SSEChatStreamEvent` + raw ext events; `McpToolProvider`(chat, `enforce_conversation_disabled=false`); `ChatHumanGate` = live pause/resume; `ChatModelResolver`). The chat **tool-approval pause→resume** orchestration (today's `should_create_user_message`/`provide_assistant_message`/`after_user_message_created`/`register_routes`) lives in this host adapter, NOT the extension trait. Behaviour-preserving.
- **ITEM-25**: Extension migration — today's `ChatExtension`s are **REWRITTEN to implement the crate `AgentExtension` trait (ITEM-32)** as server-side impls capturing `PgPool`/`Repos` in fields (a re-expression, not a lift): assistant/project/skill + memory-injection → `contribute`; `file` → a user-message content contributor; MCP tool *execution* → `ToolProvider`+`ApprovalPolicy`, tool *attachment* → `contribute`; summarization → the core `Compactor`; the streaming-delta hooks → the `AgentExtension` delta hooks. Preserve ordering + every raw SSE event.
- **ITEM-26**: Chat parity — `AgentEvent` → `SSEChatStreamEvent {started,content,complete,error}` + raw ext events; `DeltaAccumulator`-equivalent persistence via `ChatTranscriptStore`; `loop_settings.max_iteration` honored via `Budget`.

<!-- === F. Host 3 — fan-out === -->
- **ITEM-27**: `delegate` **core-injected** tool exposing `fan_out` in chat AND workflow — "research each of these N in parallel and summarize." Children inherit the user's accessible tools but NOT `delegate` (enforces `max_depth=1`), RBAC-scoped, summaries only. (Core-injected → no A8.)

<!-- === G. Config + UI === -->
- **ITEM-28**: **Admin-configurable agent policy (F5)** — a new app `agent` module: singleton `agent_admin_settings` (sandbox/approval mode, `reviewer_enabled` + `reviewer_model_id` + `reviewer_policy` + `reviewer_risk_thresholds`, per-run/per-step token caps, `default_max_steps`, `fan_out_max_threads/depth`) + **module-owned migration** + REST `GET/PUT /api/agent/settings` gated `agent::settings::{read,manage}` (`ziee_identity::PermissionCheck` — needs FOUR assoc consts `NAME`/`PERMISSION`/`DESCRIPTION`/`MODULE`; admin-only via `*` wildcard, NO grant migration; keep model+repo+cache SERVER-SIDE — copy a singleton sibling like `MemoryAdminSettings`/`SessionSettings`, NOT the `ziee-sandbox` split which was engine-forced) + a `SyncEntity` variant (app-side `ziee_framework::SyncEntityKind`) + read-at-use cache + bounds validation. Mirror the closest surviving app-side settings module.
- **ITEM-29**: Workflow **agent-step authoring UI** + **run-view agent surface** (live tool-call stream, plan/todo, the durable review-gate form pre-filled via elicit `data:`).
- **ITEM-30**: **Agent admin settings page** `/settings/agent` (mirror the closest app settings card); gated `agent::settings::read`.
- **ITEM-31**: **Plan/todo + visible-progress rendering (C3/F3)** in chat + run views (a content-type renderer + live activity stream).
- **ITEM-32**: **`AgentExtension` seam** — the TRAIT in `agent-core/src/extension.rs` (**no linkme in the crate**, DEC-18): `contribute(&mut TurnContext)`, `before_model(&mut ChatRequest) -> Flow`, `after_round(&Message) -> Flow`, + streaming-delta hooks (`on_delta`/`accumulate`/`finalize_content`). The **ziee server owns the `AGENT_EXTENSIONS` `distributed_slice`** + assembles the ordered `Vec<Arc<dyn AgentExtension>>` injected into `AgentCore::run` — **mirror `ziee_framework::entity_extension::{ExtensionEntry, ExtensionRegistry, sorted_entries}`** exactly as the chat registry already does (`chat/core/extension/registry.rs:32,337`); do NOT hand-roll. Two tiers: **core** (always-on, e.g. compaction ITEM-6) vs **feature** (opt-in). Subagents are NOT an extension (`fan_out` → the `delegate` tool).

## Crate vs host partition (the boundary)
- **`src-app/agent-core` crate (deps `ai-providers` + `ziee-core` + `ziee-identity`):** ITEM-2 (port traits), 3 (types), 4 (loop), 5 (budget/stop), 6 (`Compactor`+`CompactionExtension`), 7 (`fan_out`), 8 (plan-tool logic), 11 (ApprovalPolicy trait + matrix fn), 12 (reviewer flow), 13 (escalation logic), 27 (`delegate` tool), 32 (`AgentExtension` trait). Speaks `ai-providers` types + the crate-local `ToolResult`; the compiler-enforced port boundary is within ziee.
- **ziee server (impls + hosts + config/UI):** ITEM-1 glue, 9 (verification extension), 10 (result shaping), the concrete port impls behind 11/12/13/14/15/16/17/20, 18..23 (workflow host), 24..26 (chat host), 28..31 (config+UI), and the concrete extensions (memory/citations/project) that `impl agent_core::AgentExtension` capturing `Repos`.

## Prior-art fidelity gate (Phase 5/6/8)
`SOTA_FIDELITY.md` (carried forward) pins each prior-art invariant (INV-1..11 from LangGraph/Mastra/
Goose/Codex/Letta/DBOS/Temporal) → its ziee adaptation → the proof TEST-ID. Phase 6 adds a
`prior-art-fidelity` audit angle; Phase 8 enforces each invariant's proof test. (INV-8 is now the plain
crate-dependency-boundary test — deps `ai-providers`+`ziee-core`+`ziee-identity` — NOT an N9 grep.)

## Files to touch
**New — crate:** `src-app/agent-core/Cargo.toml` + `src-app/agent-core/src/{lib,core,ports,types,compaction,policy,reviewer,extension,fanout,budget,tokens}.rs`. Edit `src-app/Cargo.toml` (members + `[workspace.dependencies]`).
**New — server glue:** `server/src/modules/agent/{mod,settings,routes,permissions}.rs` + `modules/agent/migrations/<ts>_agent_admin_settings.sql`; `workflow/agent_dispatch.rs`; `modules/workflow/migrations/<ts>_workflow_agent_step.sql`.
**New — frontend:** `src-app/ui/src/modules/agent/` (settings) + workflow agent-step authoring/run-view + plan/todo renderer; desktop mirror where applicable.
**Edit — backend:** `modules/mod.rs`; `modules/workflow/{validate,types,runner,dispatch,cost,compiled,type_infer,ref_check,models,repository,events}.rs`; `chat/core/services/streaming.rs` + split chat extensions; `mcp/chat_extension/*`; `summarization`/`memory`/`assistant_core_memory` seams; the app-side sync surface (new `SyncEntity` variant).
**Regen:** `just openapi-regen` (new `/api/agent/settings` route + `AgentAdminSettings` type + sync entity) → BOTH `ui/` + `desktop/ui/`.

## Patterns to follow
- **Crate shape / ports** — mirror the SDK's `ziee-identity` (traits-only leaf) + `ziee-framework`'s `RequirePermissions<R: IdentityResolver>` genericity; error = `ziee_core::AppError`.
- **Loop / provider streaming** — `dispatch.rs::run_llm_call` + chat `DeltaAccumulator` for `ToolUseDelta`.
- **Tool exec + gating** — `ToolDispatcher` (→ shared `call_mcp_tool`): `resolve_tool_server`, disabled-server gates, `render_tool_arguments`, `session.call_tool`, `resource_link::persist_links`.
- **Durable gate** — `ElicitDispatcher` durable-resume + `elicit::submit_elicit` + `startup_sweep` sparing non-terminal statuses.
- **New step kind** — mirror the `Tool` kind's exhaustive arms.
- **Admin settings + permission + sync + card** — mirror the closest surviving app-side settings module (verify in Phase 2 which survived the SDK carve; `code_sandbox` settings/`session_settings`); permission via `ziee_identity::PermissionCheck`; sync via `ziee_framework::SyncEntityKind`; **module-owned migration** (N7).
- **Compaction** — `summarization/engine/summarizer.rs` (server-side, behind `replace_head`) + `conversation_summaries`.
- **Cross-app** — `ziee-control-mcp` dispatch core + app `control_mcp` module as a `ToolProvider`.
- **Tests** — unit `#[cfg(test)]` in the crate (fake ports); integration `tests/agent/` + `tests/workflow/agent_step_test.rs`; e2e for chat-on-core, the run view, the review gate, the A10 restricted-user spec.
