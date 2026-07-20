# PHASE 5 — implementation progress (tranche tracker)

Phase 5 is being built in **dependency-ordered, verified tranches** (LOCK-4: A+E → backbone D → B+C;
agent-core is a write-bottleneck so A/G/H/I can't edit it in parallel; sandbox 11/12/30/31 + url_validator
egress are cross-repo in the `sdk` submodule). Each tranche: sub-agent(s) implement file-disjoint work →
parent verifies with `cargo check`/`tsc` (trusts the artifact it runs, not the self-report) → commit.
The formal **DRIFT-N.md** + **INFRA_INTEGRATION.md** are assembled once all tranches land (drifts tracked below).

## Baseline
- `sdk` submodule initialized; build DB `:54321` reachable; node_modules symlinked; hub-seed present.
- `cargo check -p agent-core` GREEN (12.5s) and `cargo check -p ziee` GREEN (server baseline) before any change.

## Tranche status
| # | Scope | ITEMs | Verify | Commit | Status |
|---|---|---|---|---|---|
| 1 | agent-core foundation | 56(order), 38, 32 | `cargo check -p agent-core` PASS + 48/48 lib tests | b36c0d24e | ✅ VERIFIED |
| 2 | scheduler backend | 21, 22, 23 | `cargo check -p ziee` PASS (integrates T1) | 2b8e8b406 | ✅ VERIFIED |
| 3 | Group A delegate (agent-core) | 1, 3 (2 host-gate deferred) | `cargo check -p agent-core` +60/60 tests; `cargo check -p ziee` PASS | f3a9c9a85 | ✅ VERIFIED |
| 4 | Group G task-list (agent-core) | 34, 35, 36, 37 (server store impl deferred) | `cargo check -p agent-core` +68/68; `cargo check -p ziee` PASS (4 fan-in patches) | (committed) | ✅ VERIFIED |
| 5 | Backbone D core (workflow) | 14, 17, 29 (MCP tools deferred) | `cargo clean+check -p ziee` PASS (migration 202607190700; agent fixed 2 workflow_mcp fan-ins) | (committed) | ✅ VERIFIED |
| 6 | Group I compaction (agent-core) | 57, 58, 61, 63 (56-unify + server wiring deferred) | `cargo check -p agent-core` +76/76; `cargo check -p ziee` PASS (Compactor fan-in, consts deleted) | (committed) | ✅ VERIFIED |
| 7 | Server wiring consolidation | 38-srv, 61-srv(window), fan_out_max_children col, 34/35-srv store | `cargo clean+check -p ziee` PASS | (committed) | ✅ VERIFIED |
| 8 | Group H reviewer/policy security core | 39, 42, 47 (41-persist, 40, 43-46 follow-ups) | pending | — | 🔄 in progress |
| 8 | Group H reviewer/policy security core | 39, 42, 47 (41-persist, 40, 43-46 follow-ups) | `cargo check -p agent-core` +85/85; `cargo check -p ziee` PASS (agent_dispatch fan-in) | 5ebb1f0a8 | ✅ VERIFIED |
| 9 | Group F goal-seeking backend (scheduler) | 24 (FE done-when deferred) | `cargo check -p ziee` PASS | aa981b56e | ✅ VERIFIED (agent hit weekly limit during its OWN verify; impl was complete) |
| 10 | Backbone model-reachable + subagent-run lifecycle (background_mcp) | 17, 7, 9 (real LLM turn → 10b) | `cargo clean+check -p ziee` PASS (mig 202607191000; security posture parent-verified) | (committed) | ✅ VERIFIED |
| 10b | Real detached AgentCore turn (build_detached_agent_core + UnattendedDenyGate) | 7, 8, 10 | `cargo check -p ziee` + workflow::agent_step (2) + background_mcp (3) PASS (parent-run) | 0b5369ca8 | ✅ VERIFIED |
| 11 (FE) | Chat /loop + /schedule + goal-seek done-when | 18, 20, 24-FE | `tsc --noEmit` exit 0 (parent-run); fixes 4 pre-existing regen tsc errors | (committed) | ✅ VERIFIED |
| 12 (FE) | Task-list checklist + sub-agent activity renderers (presentational) | 36, 4 | `tsc` exit 0 + 7/7 unit (parent-run); NO live data yet → 12b | fd95fc0a0 | ✅ VERIFIED |
| 13 (FE) | Expose 4 orphaned admin-configurable agent settings | 24-adm, 39-adm, fan_out-adm | `tsc` exit 0 + lints (parent-run) | f3f1cfada | ✅ VERIFIED |
| 14 | Admin per-tool MCP approval override (backend) | 55-be | `cargo clean+check -p ziee` + 4 unit + integration (parent combined-run) | cd9e168ac | ✅ VERIFIED |
| 12b | Task-list SSE plumbing (event_sink map + chat SSE variant) | 36-live | `cargo check -p ziee` + tasklist_frame/compose_guard (parent combined-run) | 1793a2362 | ✅ VERIFIED |
| — | Batched openapi-regen #2 (both workspaces) | — | `openapi::tests::types_ts_parity{,_desktop}` PASS | (committed) | ✅ VERIFIED |
| 15 (FE) | Task-list live handler + per-tool approval UI | 36-live-FE, 55-FE | `tsc --noEmit` exit 0 + 3 lints (parent-run) | (committed) | ✅ VERIFIED |
| 16 | Group F inbox kind + steer note-queue (backend) | 26-be, 25-storage | `cargo clean+check -p ziee` + 3 repo DB tests (parent-run) | (committed) | ✅ VERIFIED |
| 17 | Steer loop-read seam (SteerNotePort, agent-core) | 25-loop | `cargo check -p agent-core` + -p ziee + steer unit (parent-run); amended 3 missed fan-in files | (committed) | ✅ VERIFIED |
| 18 (FE) | Agent inbox surface (notification composition) | 26-FE | pending (tsc) | — | 🔄 in progress |
| 19 | schedule_next self-paced model tool (agent-core + scheduler) | DEC-42 | pending | — | 🔄 in progress |

## Quota RESUMED 2026-07-19 — autonomous drive to 9/9
Weekly limit lifted; sub-agent tranche loop resumed. openapi-regen fan-in already batched (commit
2bc4fe8a7 — WorkflowRun, scheduler self-paced/bound/max_horizon, fan_out_max_children, goal-seeking
fields). Driving remaining ~40 items in dependency-ordered, file-disjoint tranches (1 server lane +
1 FE lane; agent-core takes the server lane exclusively), then Phases 6→9.

## Accumulated drifts (reconcile into DRIFT-N.md at Phase-5 close)
- **DRIFT (T1, impl-wins):** `Reviewer::new` kept backward-compatible + `new_with_thresholds` added (rather than changing the one server caller from another module). Server wiring TODO: `agent_dispatch.rs:787` → `new_with_thresholds(inner, policy, RiskThresholds::from_json(&settings.reviewer_risk_thresholds))`.
- **DRIFT (T1, resolved):** injection-neutralize helper placed in a new `agent-core/src/guard.rs` (DEC-80 didn't name a home).
- **DRIFT (T2, impl-wins → amend DEC-44):** self-paced self-stop sets `paused_reason='completed'` (FE badge convention, matches spent-`once` tasks) rather than null — a null would be indistinguishable from a user-disable in the UI. `is_active()` unaffected.
- **DRIFT (T10, impl-wins → SECURITY-CORRECT, overrode literal instruction):** `background` is attached via `auto_attach_builtin_ids` but deliberately NOT added to `is_builtin_server_id` (the whole-server approval-bypass). Mirrors `control_mcp` (write-capable built-in): a per-tool `is_background` arm gates the WRITE `spawn_background` while bypassing the two reads. Adding it to `is_builtin_server_id` would auto-approve detached-compute spawning in the 5 non-per-tool paths (gate.rs/agent_dispatch.rs/agent_tool_call.rs/resolver.rs/js_tool). This REVISES the generic §11 "both mcp.rs edits" rule for write-capable built-ins — parent-verified at commit.

## Deferred / TODO wiring (later tranches, tracked so nothing is silently dropped)
- **Server reviewer-thresholds wiring** (from T1 drift) — flip `agent_dispatch.rs` to `new_with_thresholds`; also wire the chat reviewer (LOCK-5, behind `ZIEE_CHAT_AGENT_CORE`).
- **Model-facing `schedule_next{delay,reason,stop}` tool** (DEC-42) that produces the self-paced proposal — the clamp + arm/write-back path is done + tested; only the read-proposal-off-the-turn wiring remains.
- **`agent_admin_settings.fan_out_max_children_per_call` column + wiring** — T3 added `SubagentLimits.max_children_per_call` (default 8) and the server literal now uses `..Default::default()`; a later tranche adds the admin column + threads it (like `fan_out_max_threads`).
- **Group G server-side durable `TaskListStore` impl** — T4 does the agent-core side (tools via the seam + port trait + re-injection extension) with a fake store; server table + migration + port impl is a follow-up.
- **openapi-regen fan-in** — DONE at commit 2bc4fe8a7 (both workspaces).
- **Real detached subagent LLM turn** (T10b, IN FLIGHT) — replace `background_mcp::execute_subagent_run`'s `minimal-placeholder` with a real `AgentCore` turn reusing `agent_dispatch.rs`'s host construction + the unattended-approval policy.
- **background_mcp integration test** — DONE (T10b added `tests/background_mcp/` key-free stub roundtrip asserting executor:'agent-core').
- **Batched openapi-regen #2** — T14 (per-tool approval REST types) + T12b (taskListChanged SSE variant + TaskItemDto) both defer regen; run `just openapi-regen` BOTH workspaces after they land, before their FE follow-ups.
- **FE follow-up: task-list live handler** — after 12b regen, register `sseEventHandlers.taskListChanged` in a chat extension → store items keyed by run/message → feed the committed `TaskListChecklist`. Gated behind `ZIEE_CHAT_AGENT_CORE` for the chat path (workflow kind:agent path streams it unconditionally).
- **FE follow-up: per-tool approval UI (ITEM-55 FE)** — after T14 regen, the tool-list + per-tool approval-mode Select on the system-MCP-server settings surface.
- **Sub-agent-activity SSE frame (ITEM-4 live, DEC-65)** — needs a NEW AgentEvent variant (agent-core) for per-child status; deferred to an agent-core tranche (T12-FE built the presentational card already).
- **Batched openapi-regen #3** — T16 (RunNote/CreateRunNote + Background.postRunNote/listRunNotes) needs regen before the steer FE. Batch with any T19 REST type. Run before steer/background-runs FE follow-ups.
- **scheduler `scheduled_task_result` notification kind** — register into NOTIFICATION_KINDS in a scheduler-owning tranche (T16 did the background_run_result one; noted so the agent-inbox filter is complete).
- **Group I unify (56/60)** + **reviewer-thresholds chat wiring** (T1 drift) + **Group C sandbox background-exec (11-13/30/31, sdk cross-repo)** — remaining agent-core/cross-repo tranches.

## Remaining tranche plan (dependency-ordered)
- A (delegate host-gate 2/4/5 chat+workflow), E FE dialog (18/20 + 24 done-when UI) [needs openapi-regen], G task-list (34-37 agent-core, shares delegate interception seam), I compaction (56 unify + 57-61,63), H approval core (39/41/42/43/44/45/46 agent-core+mcp) + H external (47-55) + H admin per-tool UI (55), F (24 goal-seek backend / 25 steer / 26 inbox / 27 event-triggers / 29 state-machine), **backbone D (14/17/29)** → then **B (7-10)** + **C sandbox (11-13/30/31, sdk cross-repo)** → I sleep-time (62).
