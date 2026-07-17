# DECISIONS — agent-core (all resolved; no open items) — SDK base

Product choices (DEC-1/2/3) were made by the human in the prior session and are **unchanged by the SDK
base** (they concern agent behavior, not platform), so they carry forward as resolved. SDK-base findings
resolve by codebase convention. Nothing remains open.

### DEC-1: Unattended-run policy for a mutating/external tool call?
**Resolution:** **Reviewer → durable gate.** `auto_review` classifies; `Low` proceeds, `High`/`Critical` escalate to the durable human-review gate. Read-only searches auto-approve, skip the reviewer.
**Basis:** user (prior session, picker).

### DEC-2: SandboxMode technical enforcement scope?
**Resolution:** **Descope per-call bwrap enforcement.** Ship the `SandboxMode` enum + approval/reviewer/escalation (the gate half); actual bwrap network/roots enforcement is a separate `code_sandbox` follow-up.
**Basis:** user (prior session) — code_sandbox has no per-call mode today.

### DEC-3: Reviewer default + model?
**Resolution:** **On by default** (`agent_admin_settings.reviewer_enabled`); fail-closed; runs only for approval-needing calls. Model = `reviewer_model_id` (nullable → the run's model), resolved via the `ModelResolver` port.
**Basis:** user (prior session) + convention (model as an admin setting).

### DEC-4: `delegate`/`update_plan` — core-injected or MCP servers?
**Resolution:** **Core-injected** into `ToolProvider::list` — NOT MCP servers; **A8 does not apply**. A child's `list` omits `delegate` (`max_depth=1`).
**Basis:** convention — loop-internal primitives, not tool servers.

### DEC-5: Per-extension disposition in the chat migration?
**Resolution:** By role: assistant/project/skill + memory-injection → `contribute` (system blocks); `file` → a user-message content contributor (`provide_user_message_content`, NOT a system block); mcp tool-execution → `ToolProvider`+`ApprovalPolicy`, tool-attach → `contribute`; summarization → the core `Compactor`; streaming-delta hooks → the `AgentExtension` delta hooks; the tool-approval pause→resume orchestration (`should_create_user_message`/`provide_assistant_message`/`after_user_message_created`/`register_routes`) → the CHAT-HOST adapter, not an extension hook.
**Basis:** codebase — the chat map + the SDK-base audit (registry hooks confirmed).

### DEC-6: Agent operational tunables — fixed or admin-configurable? (mandatory)
**Resolution:** **Admin-configurable**, a singleton `agent_admin_settings` (a new app `agent` module; model+repo+cache SERVER-side, mirror `MemoryAdminSettings`/`SessionSettings`): `default_sandbox_mode`, `unattended_approval_policy` (DEC-1), `reviewer_enabled` (DEC-3), `reviewer_model_id`, `reviewer_policy` (free text, length-capped, nullable ⇒ default), `reviewer_risk_thresholds`, `per_run_token_cap`, `per_step_token_cap`, `default_max_steps`, `fan_out_max_threads`, `fan_out_max_depth`. REST `GET/PUT /api/agent/settings` gated `agent::settings::{read,manage}`; a `SyncEntity` variant; bounds validation.
**Basis:** convention (the configurable-settings rule; `code_sandbox_settings` precedent).

### DEC-7: `default_max_steps` value?
**Resolution:** **30** (admin-configurable). The token caps are the real ceiling; Goose's 1000 is the hard failsafe.
**Basis:** convention.

### DEC-8: Agent transcript storage?
**Resolution:** **`workflow_runs.agent_transcript_json JSONB`** (workflow host; chat uses `message_contents`); `mcp_tool_calls` stays the audit journal; the transcript column is the resume source.
**Basis:** codebase — mirrors the elicit resume-field precedent.

### DEC-9: Durable gate response storage?
**Resolution:** **Reuse `elicit_response_json`** — an agent check-in IS an elicitation; consumed on resume like `ElicitDispatcher`.
**Basis:** codebase.

### DEC-10: Crash-mid-loop resume scope?
**Resolution:** **Workflow host only** (chat opts out). Gated to agent runs via `workflow_runs.resumable_agent`.
**Basis:** design + the human's "chat keeps the light durability tier".

### DEC-11: Fan-out guardrail defaults?
**Resolution:** **`max_depth=1`, `max_threads=6`** (admin-configurable); a child cannot invoke `delegate`.
**Basis:** primary source — Codex `[agents]`.

### DEC-12: Reviewer classification storage?
**Resolution:** A nullable **`mcp_tool_calls.review_classification VARCHAR(20)`** (module-owned migration).
**Basis:** codebase — additive column on the existing journal.

### DEC-13: Chat-host human gate durability?
**Resolution:** Chat keeps the **live pause / resume-on-resent-message** gate; the durable `waiting`+`resume_run` gate is workflow-only. Same `HumanGate` trait, two impls.
**Basis:** design — durability is a per-host port.

### DEC-14: Extensibility model?
**Resolution:** **Ports (6) + the `AgentExtension` pipeline** (feature plug-ins), orthogonal. Compaction is a **core (always-on) `AgentExtension`**; subagents are a tool (`delegate`), not an extension. Mirror `ziee_framework::entity_extension` for the server-owned registry.
**Basis:** codebase — generalizes `ChatExtension`; the chat registry already newtypes the SDK entity_extension.

### DEC-15: Where does the agent core live?
**Resolution:** **A ziee-app crate `src-app/agent-core`** (ziee workspace) — NOT an SDK crate. `ai-providers` stays app-side (no relocation). No N9 (may name ziee domain). Deps `ai-providers` + `ziee-core` + `ziee-identity` (direct per-crate path deps, mirror `server/Cargo.toml`).
**Basis:** user (2026-07-16) — the agent is a ziee-only feature; the SDK is the shared platform.

### DEC-16: How does the crate resolve `model_id` → `Provider`?
**Resolution:** A **6th port, `ModelResolver`**, injected into `AgentCore`. The server impl composes `create_provider_from_model_id` + the model-access RBAC. Closes the boundary leak (the crate never touches `Repos`).
**Basis:** audit (prior + SDK-base F6).

### DEC-17: Does the `call_mcp_tool` extraction change chat's disabled-server behavior?
**Resolution:** **No.** `call_mcp_tool` takes `enforce_conversation_disabled: bool` — workflow passes `true` (as today), chat passes `false` (as today). Behaviour-preserving for BOTH; the latent chat non-enforcement is tracked separately, not silently fixed.
**Basis:** codebase (audit Defect-1).

### DEC-18: Extension registry ownership?
**Resolution:** The crate defines only the `AgentExtension` **trait** (no linkme); the ziee server owns the `AGENT_EXTENSIONS` `distributed_slice` + injects the ordered `Vec<Arc<dyn AgentExtension>>` — mirroring `ziee_framework::entity_extension::{ExtensionEntry,ExtensionRegistry,sorted_entries}` (the chat registry already does).
**Basis:** audit (crate-extraction, SDK-base F7).

### DEC-A: Accept `ziee-core`-transitive axum + sqlx in the crate?
**Resolution:** **Yes.** Depending on `ziee-core` for `AppError`/`ApiResult` pulls axum + sqlx (types-only) into `agent-core`'s tree. "Build-DB-free / no sqlx" means **no `query!` macros** (no build DB), NOT a leaf dependency tree. This is the accepted cost of the app-wide `AppError` return type (the alternative — a bespoke crate error — was rejected for app-consistency, DEC-15/SDK convention).
**Basis:** codebase (SDK-base audit F2) — `ziee-core` deps axum/sqlx/dirs.

### DEC-B: Where does `ModelResolver` source its two halves?
**Resolution:** The server impl imports `create_provider_from_model_id` (`chat::core::ai_provider`, global-`Repos`-coupled) + `user_has_access_to_provider`/`validate_model_access` for RBAC. The workflow→chat import is **accepted** (precedented — `workflow/runner.rs` already calls `create_provider_from_model_id`), NOT relocated in this feature. A TEST (TEST-41) asserts `resolve` DENIES an inaccessible model.
**Basis:** codebase (SDK-base audit F6) — precedent + minimal-change.

### DEC-19: How is the per-request extension payload (`SendMessageRequest.extensions`) surfaced to ported extensions?
**Resolution:** `AgentTurnRequest` gains an opaque `inputs: serde_json::Value` (default `Null`), surfaced on `TurnContext.inputs`. Ported context-injectors read their own key out of it (attach flags, `file_ids`, `tool_approvals`) exactly as they read `SendMessageRequest.extensions` today. The crate never names a chat field — it carries an opaque bag, keeping the loop domain-neutral (the workflow host passes `Null`).
**Basis:** design — mirrors chat's existing `SendMessageRequest.extensions: serde_json::Value` bag; keeps N-neutrality of the crate.

### DEC-20: How do ported extensions emit SSE (chat's `tx`)?
**Resolution:** The trait stays sink-free. A concrete server-side ported extension that must emit (title updates, approval-required) captures the `EventSink` (or the raw SSE `ext_tx`) in its OWN struct field at construction, and emits through it — identical to how the workflow port impls capture `Repos`. No `AgentExtension` signature change.
**Basis:** codebase — same pattern as the workflow `WorkflowEventSink`/port structs capturing state; DEC-15 (concrete impls may capture server state).

### DEC-21: Extension ordering under the re-home?
**Resolution:** `AgentExtension::order()` is the sole sort key (ascending), seeded from each ported extension's existing chat `METADATA.order` so the relative order is byte-identical (assistant before project before mcp, etc.). The server builds the ordered `Vec<Arc<dyn AgentExtension>>` once; the crate does not sort.
**Basis:** codebase — preserves the current `auto_register_extensions` order (chat/mod.rs) exactly.

### DEC-22: Who owns the chat message lifecycle (user-message create / assistant-message reuse / resume) after the re-home?
**Resolution:** The **chat host** (the `send_message` handler + `ChatAgentDispatcher`), NOT the crate. `should_create_user_message` / `provide_assistant_message` / `after_user_message_created` are message-lifecycle decisions the host makes BEFORE calling `core.run` (they drive cross-request approval resume). The crate loop only sees an already-seeded transcript (`TurnSeed::NewMessage` for a fresh turn, `TurnSeed::Resume` for an approval resume). This keeps the fire-and-forget POST contract + the per-`assistant_message_id` cancel token host-side.
**Basis:** design — these hooks name chat-only types (`SendMessageRequest`, `Message`); they cannot be domain-neutral, so they stay in the host (matches the map's resume semantics).
