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
| 5 | Backbone D core (workflow) | 14, 17, 29 (MCP tools deferred) | pending | — | 🔄 in progress |

## Accumulated drifts (reconcile into DRIFT-N.md at Phase-5 close)
- **DRIFT (T1, impl-wins):** `Reviewer::new` kept backward-compatible + `new_with_thresholds` added (rather than changing the one server caller from another module). Server wiring TODO: `agent_dispatch.rs:787` → `new_with_thresholds(inner, policy, RiskThresholds::from_json(&settings.reviewer_risk_thresholds))`.
- **DRIFT (T1, resolved):** injection-neutralize helper placed in a new `agent-core/src/guard.rs` (DEC-80 didn't name a home).
- **DRIFT (T2, impl-wins → amend DEC-44):** self-paced self-stop sets `paused_reason='completed'` (FE badge convention, matches spent-`once` tasks) rather than null — a null would be indistinguishable from a user-disable in the UI. `is_active()` unaffected.

## Deferred / TODO wiring (later tranches, tracked so nothing is silently dropped)
- **Server reviewer-thresholds wiring** (from T1 drift) — flip `agent_dispatch.rs` to `new_with_thresholds`; also wire the chat reviewer (LOCK-5, behind `ZIEE_CHAT_AGENT_CORE`).
- **Model-facing `schedule_next{delay,reason,stop}` tool** (DEC-42) that produces the self-paced proposal — the clamp + arm/write-back path is done + tested; only the read-proposal-off-the-turn wiring remains.
- **`agent_admin_settings.fan_out_max_children_per_call` column + wiring** — T3 added `SubagentLimits.max_children_per_call` (default 8) and the server literal now uses `..Default::default()`; a later tranche adds the admin column + threads it (like `fan_out_max_threads`).
- **Group G server-side durable `TaskListStore` impl** — T4 does the agent-core side (tools via the seam + port trait + re-injection extension) with a fake store; server table + migration + port impl is a follow-up.
- **openapi-regen fan-in** — after the backend-type tranches (scheduler already added `bound_conversation_id`/`?conversation_id`/`schedule_kind:self_paced`/`max_horizon_days`), run `just openapi-regen` BOTH workspaces before the FE tranches consume the types.

## Remaining tranche plan (dependency-ordered)
- A (delegate host-gate 2/4/5 chat+workflow), E FE dialog (18/20 + 24 done-when UI) [needs openapi-regen], G task-list (34-37 agent-core, shares delegate interception seam), I compaction (56 unify + 57-61,63), H approval core (39/41/42/43/44/45/46 agent-core+mcp) + H external (47-55) + H admin per-tool UI (55), F (24 goal-seek backend / 25 steer / 26 inbox / 27 event-triggers / 29 state-machine), **backbone D (14/17/29)** → then **B (7-10)** + **C sandbox (11-13/30/31, sdk cross-repo)** → I sleep-time (62).
