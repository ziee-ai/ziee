# DRIFT round 1 — implementation vs plan/design

Reconciled each item against PLAN + the design's invariants as it landed.

- **DRIFT-1.1** — verdict: impl-wins — **the migration adds `parent_conversation_id`
  (FK `conversations` ON DELETE CASCADE), not just `parent_message_id`.** DEC-7 planned a
  single `parent_message_id REFERENCES messages(id) ON DELETE CASCADE` for the cascade. But
  verifying by RUNNING the schema showed `messages` has NO FK to `conversations` and
  `delete_conversation` relies on FK cascade — so a message-FK could NOT guarantee a child
  cascades when its CONVERSATION is deleted (the DEC-3 promise). Resolution: `parent_message_id`
  is now the plain QUERY key (no FK) and a dedicated `parent_conversation_id` (ON DELETE
  CASCADE to `conversations`) is the lifecycle guarantee. DEC-7/DEC-8 amended; TEST-8 asserts
  the conversation-delete cascade (count 1→0), which PASSES. Upholds INV (DEC-3) more robustly
  than the plan.

- **DRIFT-1.2** — verdict: impl-wins — **TEST-10 asserts the owner-scoped refetch target, not a
  captured SSE frame.** TESTS.md TEST-10 planned "settle emits a `WorkflowRun` sync observed via
  `SyncProbe`." Proven infeasible: the integration test drives `settle_child` IN THE TEST PROCESS,
  whose process-global sync registry is separate from the spawned SERVER's (which `SyncProbe`'s
  SSE connects to), and an in-process axum SSE `Event` exposes no data getter to parse the frame.
  Resolution: TEST-10 asserts the SUBSTANTIVE, deterministic half of notify-and-refetch — after
  settle the OWNER's refetch endpoint shows the child terminal and a foreign user is 404
  (never in the owner audience → never notified). The `emit_workflow_run` call itself is the SAME
  owner-scoped emit the background terminal path uses; live SSE delivery is covered by the
  real-fan-out e2e (TEST-13). TESTS.md TEST-10 line amended to match. INV-4 upheld.

- **DRIFT-1.3** — verdict: none — the `ChildSink` port shipped exactly as DEC-10 (no
  `ChildParentRef`; host bakes parent identity into the concrete factory) and the detail endpoint
  reuses `get_background_run_detail` exactly as DEC-6. No divergence.

- **DRIFT-1.4** — verdict: none — `BackgroundRunDetail.activity` shipped as `Vec<ProgressKind>`
  (DEC-13), matching the FE `AgentActivityEntry = Extract<ProgressKind,{type:'agent_activity'}>`;
  `get_background_run_detail` rewritten to `sqlx::query!` + manual build as planned. Verified
  end-to-end (TEST-1 reads thinking+tool+message back through the endpoint).

- **DRIFT-1.5** — verdict: impl-wins — **TEST-12 seeds the background run's activity via
  deterministic SQL, not a live-LLM background turn.** It still drives the REAL backend (the
  seeded transcript lives in `step_logs_json['agent::agent_activity']` — the exact projection
  `GET /api/background/runs/{id}` reads — with NO API mocking), mirroring the sibling
  `background-in-conversation.spec.ts`. This makes the render + reload-durability + activity-less
  cases deterministic and always-run, instead of gating on a minimal LLM reply that might record
  a single entry. INV-1 upheld (the endpoint→timeline path is exercised for real).

- **DRIFT-1.6** — verdict: impl-wins — **TEST-13 asserts child-drill-in durability at the
  transcript SOURCE (the durable `/api/subagent-runs/{id}` REST), not by the live card
  reappearing after reload.** Verified in code that the `subAgentActivity` frame is SSE-only with
  NO replay / persisted content block (the live card is ephemeral by design, per its own
  doc-comment) — so ITEM-9's durability IS the on-demand REST re-fetch of each child transcript,
  which is exactly what the test proves survives reload. The always-run 404→status-only transition
  is covered deterministically by TEST-14 (unit). INV-2 upheld.

**Unresolved drifts:** 0
