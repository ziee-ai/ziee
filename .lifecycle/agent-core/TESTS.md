# TESTS — agent-core (every item covered; bipartite item↔test map; SDK base)

## Testing strategy — EXTRACTION + MIGRATION, not only new code
Two modes: **(1) new capability** (TEST-1..37,41) unit/integration/e2e; **(2) behaviour-preservation**
for the paths this refactors (chat loop → core; `ToolDispatcher` → shared `call_mcp_tool`; chat
extensions → `AgentExtension`) — guarded by: the **existing chat + workflow suites run UNCHANGED**
(TEST-38/39/40, the strongest gate — never edit an existing test to accommodate the migration); explicit
parity assertions (TEST-16/24/25); a characterization golden (TEST-24). Plus the SOTA-fidelity behavioral
invariants (`SOTA_FIDELITY.md`).

## Unit (crate, fake ports — no external deps)
- **TEST-1** (tier: unit) [covers: ITEM-1, ITEM-2] file: `src-app/agent-core/src/ports.rs` — asserts: the six port traits are object-safe (`Arc<dyn …>` compile) and in-memory fakes implement them.
- **TEST-2** (tier: unit) [covers: ITEM-3] file: `src-app/agent-core/src/types.rs` — asserts: `AgentEvent`/`ReviewDecision`/`StopReason`/`Decision`/`ToolResult` serde roundtrip + all variants present.
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/agent-core/src/core.rs` — asserts: with a fake provider emitting `ToolUse` then a final message, the loop calls `ToolProvider::call`, appends the result, re-invokes, stops on `NoToolCall`.
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/agent-core/src/core.rs` — asserts: `IterationCap` synthesizes `is_error` for unexecuted `ToolUse`s; `TokenCap` aborts; cancel → `Stopped(Halted)`.
- **TEST-5** (tier: unit) [covers: ITEM-6] file: `src-app/agent-core/src/compaction.rs` — asserts: `Compactor::fit` over budget summarizes oldest ~30% + keeps newest, emits `HistoryReplaced`, keeps pinned blocks verbatim, no-op under budget (crate-local `estimate_tokens`).
- **TEST-6** (tier: unit) [covers: ITEM-7] file: `src-app/agent-core/src/fanout.rs` — asserts: `fan_out` runs N children bounded by `max_threads`, returns `SubagentSummary`s (never transcripts), child `ToolProvider::list` excludes `delegate` (`max_depth=1`).
- **TEST-7** (tier: unit) [covers: ITEM-8] file: `src-app/agent-core/src/core.rs` — asserts: the `update_plan` tool records the todo list + emits a plan `AgentEvent`.
- **TEST-8** (tier: unit) [covers: ITEM-9] file: `src-app/agent-core/src/extension.rs` — asserts: a grounding `AgentExtension` contributes the nudge system block when its tools are in scope (crate stays domain-free — the nudge text is injected).
- **TEST-9** (tier: unit) [covers: ITEM-10] file: `src-app/agent-core/src/core.rs` — asserts: the `concise|detailed` convention truncates a concise (default) result vs detailed, without breaking `structured_content`.
- **TEST-10** (tier: unit) [covers: ITEM-11] file: `src-app/agent-core/src/policy.rs` — asserts: `ApprovalPolicy::decide` matrix — read-only/trusted → `Auto`; mutating under `OnRequest` → `Prompt`; `Never` → deny-return; `UnlessTrusted` auto only read-only.
- **TEST-11** (tier: unit) [covers: ITEM-12] file: `src-app/agent-core/src/reviewer.rs` — asserts: classification `Low→Auto`/`High→Prompt`/`Critical→Deny`; any reviewer error is **fail-closed** to `Deny`.
- **TEST-12** (tier: unit) [covers: ITEM-13] file: `src-app/agent-core/src/policy.rs` — asserts: escalation builds a `GateAsk` with proposed widened perms; `ApprovedForSession` records an allow-rule consulted next.
- **TEST-13** (tier: unit) [covers: ITEM-16] file: `src-app/agent-core/src/core.rs` — asserts: idempotency key `<run_id>:<turn>:<ordinal>`; a resume reloading a transcript with a tool_result does NOT re-invoke `ToolProvider::call`.
- **TEST-34** (tier: unit) [covers: ITEM-32, ITEM-6] file: `src-app/agent-core/src/extension.rs` — asserts: extensions run in `order`; `contribute` adds a system block + extends tool scope; `before_model` can mutate + short-circuit (`Flow`); `after_round` fires per round; a core extension (compaction) is always present even in a minimal set; the delta hooks fire.
- **TEST-36** (tier: unit) [covers: ITEM-1] file: `src-app/agent-core/tests/deps_boundary.rs` — asserts: (INV-8) the `agent-core` crate's resolved deps = {`ai-providers`, `ziee-core`, `ziee-identity`} (+ their transitives) and **EXCLUDE the `ziee` server crate** and any app module — the compiler-enforced port boundary within ziee.

## Integration (server, real runner / DB / MockMcpServer)
- **TEST-14** (tier: integration) [covers: ITEM-14] file: `src-app/server/tests/agent/journal_test.rs` — asserts: a tool call in an agent run writes an `mcp_tool_calls` row linked to the run (`workflow_run_id`), sanitized `result_json`.
- **TEST-15** (tier: integration) [covers: ITEM-18, ITEM-19, ITEM-23] file: `src-app/server/tests/workflow/agent_step_test.rs` — asserts: a `kind: agent` step runs a real loop via `MockMcpServer`, calls a tool, produces a typed output; `require_model` enforced; `cost.rs` dry_run has an `Agent` arm.
- **TEST-16** (tier: integration) [covers: ITEM-20, ITEM-21] file: `src-app/server/tests/workflow/agent_step_test.rs` — asserts: `call_mcp_tool(enforce_conversation_disabled=true)` (workflow, as today) fail-closes a disabled server; `=false` (chat, as today) does NOT — the extraction flips NEITHER path (DEC-17).
- **TEST-17** (tier: integration) [covers: ITEM-15, ITEM-17] file: `src-app/server/tests/workflow/agent_step_resume_test.rs` — asserts: (a) durable gate → `waiting` → `resume_run` after simulated restart → submit → completes; (b) a crashed `running` agent run is marked `resumable` (not `failed`) via `resumable_agent`, and `resume_run` finishes it.
- **TEST-18** (tier: integration) [covers: ITEM-16] file: `src-app/server/tests/workflow/agent_step_resume_test.rs` — asserts: (INV-2) on resume the transcript (with completed tool_results) reloads; the completed tool is NOT re-called (`MockMcpServer` count unchanged); an in-flight call re-runs once.
- **TEST-20** (tier: integration) [covers: ITEM-28] file: `src-app/server/tests/agent/settings_test.rs` — asserts: `GET/PUT /api/agent/settings` roundtrip, bounds validation → 400, cache invalidation, `sync_publish(SyncEntity::AgentAdminSettings)` emit.
- **TEST-21** (tier: integration) [covers: ITEM-28] file: `src-app/server/tests/agent/settings_test.rs` — asserts: **A9 backend deny** — a non-admin without `agent::settings::manage` → 403 on PUT; without `::read` → 403 on GET.
- **TEST-22** (tier: integration) [covers: ITEM-12] file: `src-app/server/tests/agent/reviewer_test.rs` — asserts: a mutating tool call under a headless policy triggers the reviewer; `High` escalates to the durable gate; the classification persists to `mcp_tool_calls.review_classification`; a configured `reviewer_policy` reaches the reviewer prompt (empty ⇒ default).
- **TEST-23** (tier: integration) [covers: ITEM-9] file: `src-app/server/tests/agent/verification_test.rs` — asserts: an agent with the `citations` tool, given a fabricated identifier, surfaces `not_found` rather than inventing (mock resolver).
- **TEST-24** (tier: integration) [covers: ITEM-24, ITEM-26] file: `src-app/server/tests/chat/agent_core_parity_test.rs` — asserts: a chat send-message on the core emits the same `SSEChatStreamEvent` sequence + persists the same `message_contents` as the pre-migration path (scripted provider); the golden INCLUDES a tool-approval pause→resume + a custom-delta extension.
- **TEST-25** (tier: integration) [covers: ITEM-25] file: `src-app/server/tests/chat/extension_split_test.rs` — asserts: the extension migration preserves ordering (summarization before memory) + assistant→project layering; the existing `extension_registration.rs` ordering contract still passes.
- **TEST-27** (tier: integration) [covers: ITEM-22] file: `src-app/server/tests/agent/migration_test.rs` — asserts: the module-owned migrations apply — `workflow_runs.agent_transcript_json` writable, `resumable` status + `resumable_agent` accepted, `mcp_tool_calls.review_classification` present, `agent_admin_settings` singleton seeded.
- **TEST-41** (tier: integration) [covers: ITEM-2, ITEM-20] file: `src-app/server/tests/agent/model_resolver_test.rs` — asserts: (F6) the server `ModelResolver` impl resolves an accessible `model_id` → a `Provider`, and **DENIES a `model_id` the user lacks access to** (`user_has_access_to_provider`=false → error) — the RBAC-bound per-child/reviewer provider minting.
- **TEST-37** (tier: integration) [covers: ITEM-15] file: `src-app/server/tests/workflow/agent_step_resume_test.rs` — asserts: (INV-3, Mastra) the durable snapshot is written only at gate/completion boundaries, NOT per streamed token.

## E2E
- **TEST-31** (tier: e2e) [covers: ITEM-30] file: `src-app/ui/tests/e2e/settings/agent-settings.spec.ts` — asserts: an admin opens `/settings/agent`, edits sandbox/approval/reviewer/caps, saves, values persist across reload.
- **TEST-32** (tier: e2e) [negative-perm] [covers: ITEM-28] file: `src-app/ui/tests/e2e/settings/agent-settings-negperm.spec.ts` — asserts: a user LACKING `agent::settings::read` sees NO `/settings/agent` nav entry, the route is guarded, no agent-settings card renders (A10, all four gating layers).

## Regression / parity gate (behaviour-preservation for the migrated paths)
- **TEST-38** (tier: integration) [covers: ITEM-24, ITEM-25] file: `src-app/server/tests/chat/` (existing suite) — asserts: the FULL existing chat integration suite passes UNCHANGED after chat migrates onto the core (edits forbidden).
- **TEST-40** (tier: integration) [covers: ITEM-21] file: `src-app/server/tests/workflow/` (existing tool-step suite) — asserts: the existing workflow `kind: tool` suite passes unchanged after the `ToolDispatcher → call_mcp_tool` extraction.

## Coverage check (every ITEM → ≥1 TEST)
1→T1,T36 · 2→T1,T41 · 3→T2 · 4→T3 · 5→T4 · 6→T5,T34 · 7→T6,T19 · 8→T7 · 9→T8,T23 · 10→T9,T26 ·
11→T10 · 12→T11,T22 · 13→T12 · 14→T14 · 15→T17,T37 · 16→T13,T18 · 17→T17 · 18→T15 · 19→T15 ·
20→T16,T41 · 21→T16,T40 · 22→T27 · 23→T15 · 24→T24,T28,T38,T39 · 25→T25,T38 · 26→T24 ·
27→T19,T29 · 28→T20,T21,T32 · 29→T30 · 30→T31 · 31→T28,T30,T33 · 32→T34. All covered.
UI items (24,26,27,29,30,31) have e2e; the new `agent::settings` permission has A9 (T21) + A10 (T32).
