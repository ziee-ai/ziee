# DRIFT-1 — Phase-5 implementation drifts (consolidated)

Drifts recorded across the 42 verified implementation tranches (see PHASE5_PROGRESS.md for the
per-tranche log). Each was resolved to convergence — every one is `resolved` or `impl-wins` with the
plan/DECISIONS reconciled below. No plan-wins reverts were required.

**Unresolved drifts:** 0

## Drift entries

- **DRIFT-1.1** — verdict: resolved — `Reviewer::new` kept backward-compatible + `new_with_thresholds` added (T1) rather than editing the sole server caller from another module. Server wiring landed in T7 (both detached hosts route through `build_detached_agent_core` → `new_with_thresholds(RiskThresholds::from_json(settings.reviewer_risk_thresholds))`); the "still inert" note was STALE. T31 added the binding regression test (`admin_thresholds_change_reviewer_decision`), so the admin setting provably changes decisions.

- **DRIFT-1.2** — verdict: resolved — the injection-neutralize helper (DEC-80, which named no home) was placed in a new `agent-core/src/guard.rs` (T1). Consumed by the child-summary scan; no ambiguity remains.

- **DRIFT-1.3** — verdict: impl-wins — self-paced self-stop sets `paused_reason='completed'` (T2) rather than null (amends DEC-44): a null is indistinguishable from a user-disable in the FE badge; `is_active()` unaffected. The FE loop-card (T22) renders this as a green "Completed"/"Loop finished".

- **DRIFT-1.4** — verdict: impl-wins — the `background` built-in MCP server is attached via `auto_attach_builtin_ids` but deliberately NOT added to `is_builtin_server_id` (the whole-server approval-bypass) (T10). Mirrors `control_mcp` (write-capable built-in): a per-tool `is_background` arm gates the WRITE `spawn_background` while bypassing the reads. Adding it to `is_builtin_server_id` would auto-approve detached-compute spawning in 5 non-per-tool paths — a security hole. REVISES the generic CODING_GUIDELINES §11 "both mcp.rs edits" rule for write-capable built-ins.

- **DRIFT-1.5** — verdict: impl-wins — steer note-queue is a DURABLE table `background_run_notes` (T16), amending DEC-79's in-memory RunHandle queue: a detached run's REST-enqueue + agent-core loop-read cross a process boundary and must survive restart (durable-resume-aligned). DEC-79's depth-8 drop-oldest + owner-only semantics are preserved; iteration-boundary delivery is the T17 loop-read seam (`SteerNotePort`).

- **DRIFT-1.6** — verdict: impl-wins — schedule_next's model-supplied free-text `reason` is recorded in agent-core's `ScheduleProposal` but dropped in the scheduler's `SelfPacedProposal` conversion (T19): the clamp/write-back has no `reason` field (its `reason` arg is a status enum). Surfacing the reason on the run row is DEC-43, a separate concern, left untouched.

- **DRIFT-1.7** — verdict: impl-wins — ITEM-56 (unify summarizers) landed the unification core (T26): ONE `SUMMARY_PROMPT_9_SECTION` + one `Summarizer` port/assembly shared by the agent-core Compactor Tier-4 AND the server rolling-summary engine; plus the extension `.order()` fix (T1) that ITEM-56 also called for. DEC-128's further step — making `conversation_summaries` the SOLE writer / making chat `replace_head` a no-op / retiring the engine's independent rolling summary — is DEFERRED: there is NO live double-write today (the legacy and agent-core chat loops are mutually exclusive, so the "double-write bug" ITEM-56 cites cannot manifest on the default path), and the retirement only matters once the agent-core chat path becomes default (behind `ZIEE_CHAT_AGENT_CORE`). The `Compactor::with_summarizer` seam is in place for that cutover. Recorded as a documented partial tied to the LOCK-5 chat cutover, not a silent cut.

- **DRIFT-1.8** — verdict: impl-wins — `AgentAdminSettings::top_level_allow_delegate(Option<&Self>)` helper extracted (T33) rather than duplicating the inline `settings.map(|s| s.delegate_enabled).unwrap_or(false)` at both hosts — one tested source of truth, mirroring the `reviewer_thresholds` helper precedent. Semantically identical (fail-closed on a missing settings row).

- **DRIFT-1.9** — verdict: resolved — `background::use` was absent from the generated `Permissions` enum because T16/T21's `.response_with::<403,()>("Missing background::use")` route-doc lines CLOBBERED the structured 403 that the spec-driven permission collector reads. Root-caused + fixed in T32 (removed the 5 clobbering lines); regen #7 restored `Permissions.BackgroundUse`; T34 swapped the FE raw-string workaround to the enum member. No workaround remains.
