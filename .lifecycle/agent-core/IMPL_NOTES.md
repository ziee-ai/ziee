# IMPL_NOTES — Phase-5 running record (not a gate artifact)

Durable notes across parallel-implementation waves; each item is verified by the
orchestrator (cargo re-run), not trusted from a sub-agent self-report (P1).

## Wave 1 — DONE + orchestrator-verified
- **Crate complete (ITEM-1..8,11,12 core pieces).** `src-app/agent-core/src/{core,compaction,fanout,reviewer,test_fakes}.rs` + foundation. `cargo test -p agent-core` → **35 passed** (re-run by orchestrator). The loop wires the real ports; a `ModelClient` seam (real `chat_stream`-wrapping impl + `ScriptedModel` fake) makes it network-free-testable. Accepted sub-agent deviations: `AgentCore::run` returns `Result<Vec<AgentEvent>, AppError>` (surfaces port errors); `fan_out` takes a leading `user_id` (for RBAC `ModelResolver`); added a `ModelClientFactory` seam.
- **Agent admin-settings backend (ITEM-28).** `server/src/modules/agent/` + migration `202607160100_agent_admin_settings.sql` + `SyncEntity::AgentAdminSettings` + `Repos.agent` + `modules/mod.rs`. `cargo check -p ziee` → **exit 0** (re-run by orchestrator; the 2 warnings are pre-existing `mcp`/`auth` dead-code, not this module).

## Wave 2 — DONE + orchestrator-verified (workflow kind:agent host, ITEM-18..23)
- `workflow/agent_dispatch.rs` (5 port impls + `AgentDispatcher` building+running `AgentCore`), `StepConfig::Agent` + all exhaustive arms, `call_mcp_tool` extraction (DEC-17), migrations `202607170100_workflow_agent_step` + `202607170105_mcp_review_classification`, crate wired into `server/Cargo.toml`. **Verified by orchestrator:** `cargo test -p ziee --lib workflow::` → **159 passed / 0 failed**, `cargo check -p ziee` green; `AgentDispatcher` confirmed to build `AgentCore{…}` + `core.run(...)`.
- **Deferred to stage 3 (safety+durability) — NOT yet functional (honest gaps):**
  - ITEM-12 reviewer: `AgentDispatcher` sets `reviewer: None` (a `Review` decision escalates straight to the human gate — the crate's safe default). The reviewer subagent is not wired into the workflow host yet.
  - ITEM-16 resume-replay: `WorkflowTranscriptStore::journal_tool_call` is a no-op + `completed_tool_calls` returns empty (the `mcp_tool_calls` row is already written by the `McpSession::call_tool` chokepoint). Resume currently relies on reloading `agent_transcript_json` (design DEV-4/INV-2). The idempotency-key replay path is NOT wired.
  - ITEM-17 crash-resume: the migration widens the status CHECK to `resumable` + `set_resumable_agent` exists, but `startup_sweep`/`resume_run` are NOT yet wired to mark+resume crashed agent runs; no `WorkflowRunStatus::Resumable` enum variant (treated as a non-terminal string).
- **ToolDispatcher refactor edge-case (for the parity audit):** arg rendering now runs BEFORE the disabled-server gate (the gate moved into `call_mcp_tool`), so error PRECEDENCE differs only on doubly-invalid input; happy-path + single-failure is byte-identical. **TEST-40 (integration parity) is owed at Phase 8** to confirm.
- **Owed integration tests (need the harness — Phase 8):** TEST-15 (`kind: agent` step runs), TEST-16/40 (`call_mcp_tool` parity), TEST-17 (durable gate + crash resume).

## Open integration items (for later stages / drift)
- **DRIFT-CANDIDATE — enum vocab mismatch.** The crate's `types::SandboxMode`/`ApprovalMode` serialize PascalCase (no `rename_all`), but `202607160100_agent_admin_settings.sql`'s CHECK constraints use kebab (`read-only`/`workspace-write`/`danger-full-access`; `untrusted`/`on-failure`/`on-request`/`never`). Not yet wired (settings store strings; the crate enums aren't read from settings until ITEM-11's settings↔core wiring). RECONCILE when wiring: add `#[serde(rename_all="kebab-case")]` (+ `#[serde(alias=...)]` for Codex's `untrusted`↔`UnlessTrusted`) to the crate enums, or align the CHECK values. Confirm the two agree with a roundtrip test.
- **OpenAPI regen owed** — the new `/api/agent/settings` route + `AgentAdminSettings` types + the sync entity need `just openapi-regen` (BOTH `ui/`+`desktop/ui/`) — deferred to the UI stage (ITEM-30).

## Remaining Phase-5 stages (build order)
2. Workflow host (ITEM-18..23): `StepConfig::Agent`, `AgentDispatcher`, the workflow port impls (`McpToolProvider`/`WorkflowEventSink`/`WorkflowTranscriptStore`/`WorkflowHumanGate`/`WorkflowModelResolver`), module-owned migration (`agent_transcript_json`/`resumable_agent`/`review_classification`), `cost.rs` arm. Wire the crate into the server (`agent-core` dep on `ziee`).
3. Safety+durability wiring (ITEM-13,16,17) into the workflow host.
4. Chat host migration (ITEM-24..26) — the largest/riskiest; behaviour-preserving, guarded by the existing chat suites.
5. Fan-out surface (ITEM-27) + config/UI (ITEM-29,30,31) + openapi-regen.
Then the two mandatory Phase-5 walks per item + DRIFT-to-0, then phases 6–9.
