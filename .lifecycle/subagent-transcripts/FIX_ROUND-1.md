# Fix round 1

Two BLIND angles ran (design-conformance + security/correctness), disjoint areas,
NO overlap and NO high-severity finding — so by the corroboration rule (work =
corroborated-by-≥2 OR oracle-confirmed OR severity security/data-loss/authz) there
are **zero promotable findings**. All five ledger rows are low, single-angle. Each
was triaged + dispositioned:

- **SEC (clean)** — both `/api/subagent-runs` endpoints are `RequirePermissions<(WorkflowsRead,)>`
  + owner-scoped (foreign → 404). Rejected (not a defect).
- **SEC-2 (low, wontfix)** — `get_subagent_run` reuses `get_background_run_detail`, so a user's
  OWN non-subagent background run is also fetchable via `/api/subagent-runs/{id}`. Owner-scoped,
  same DTO + perm the user already holds via `/api/background/runs` — no leak/escalation.
  Deliberate reuse (DEC-6); accepted.
- **CORR-1 (low, documented)** — the `FailFast` early-return path does not settle already-spawned
  survivors. UNREACHABLE today (FailFast is factory-free by contract — its only caller, the public
  `fan_out`, never injects a factory; the chat factory path uses ErrorSummary, which settles every
  child). Addressed by an explicit INVARIANT doc-comment on `settle_child_run` (fanout.rs) stating
  the contract + the fix a future factory+FailFast host would need. No unreachable code added.
- **DC-1 (low, wontfix)** — TEST-10 asserts the owner-scoped refetch target, not the SSE emission
  itself. This is the DRIFT-1.2 tradeoff (an in-process axum SSE `Event` frame is not capturable);
  the emission is on the real path and reuses the SyncProbe-tested `emit_workflow_run`; live SSE
  delivery is covered by the real-fan-out e2e (TEST-13). Accepted.
- **DC-2 (low, wontfix)** — INV-2's "tool results" wording slightly over-states the reused stream,
  which captures ToolUse + ToolNotification (byte-identical to the workflow `AgentActivityTimeline`
  the design mandates reusing). Changing it would diverge the workflow host + break TEST-7.
  Conforms to the sanctioned reference stream; surfaced to the human (HUMAN_FEEDBACK candidate).

No behavior defect, no authz leak, no reachable panic (the security/correctness angle verified
map_agent_event byte-parity, the query-rewrite field mapping, join-barrier index alignment, and
all 8+ AgentCore construction sites). The audit's own summary: "0 real defects."

**New confirmed findings:** 0
