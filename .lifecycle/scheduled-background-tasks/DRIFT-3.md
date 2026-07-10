# DRIFT-3 — rebase onto current origin/main + lifecycle-hardening reconciliation

After rebasing (merging) onto current `origin/main` — which advanced with js-tool,
citations, and the **lifecycle hardening** (merge-gate + A1–A9 checks) — two
divergences surfaced from the hardened validator. Both resolved.

## Drifts

- **DRIFT-3.1** — verdict: resolved — **migration collision with js-tool.** Main
  took migrations `133/134/135` (js-tool). My scheduler migrations were `133-138`.
  Renumbered to **`139-144`** (order preserved: `create_scheduled_task_runs` (144)
  still follows `create_scheduled_tasks` (139) for its FK). Verified against
  CURRENT main (max = 135), not memory. All FKs are by table name, not number, so
  order is the only constraint. Updated the 3 doc-comment references
  (permissions.rs ×2, crud_test.rs) and PLAN.md paths. `just openapi-regen`
  equivalent re-run so `types.ts` (both workspaces) carries BOTH `JsToolSettings*`
  (main) AND `ScheduledTask*`/`Notification*` (mine).

- **DRIFT-3.2** — verdict: plan-wins — **the A5 shrink-guard rejected the
  consolidated 28-test plan.** DRIFT-2.2 had renumbered the enumeration from 44 to
  28 real tests. The hardening's A5 forbids a previously-committed TEST-ID from
  vanishing (don't shrink to pass). **Resolution:** TESTS.md restored to all 44
  TEST-IDs, each mapped to the REAL implemented test — consolidated planned tests
  point to the broader test that asserts the behavior at the tier where it is
  genuinely exercised (a planned unit test proven end-to-end at integration tier;
  a planned integration test whose pure logic is unit-tested). No behavior was
  dropped; the mapping is explicit in TESTS.md. Every TEST-ID has a real PASS.

- **DRIFT-3.3** — verdict: plan-wins — **ITEM-32 (continue-in-chat) RE-SCOPED and
  implemented.** DRIFT-2.1 had descoped it. Under A5 its tests (TEST-35 / TEST-37)
  cannot vanish and must PASS, so the feature is now built:
  `POST /api/scheduled-tasks/runs/{run_id}/continue` opens a NEW conversation
  seeded with the run's context (owner-scoped, cross-user 404), plus a
  "Continue in chat" button on each run row. Restored as ITEM-32 in PLAN.md; the
  descope note removed.

**Unresolved drifts:** 0
