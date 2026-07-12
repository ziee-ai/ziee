# DRIFT-1 — implementation vs plan

Reconciling the shipped code against PLAN.md / TESTS.md / DECISIONS.md after the
implementation pass.

- **DRIFT-1.1** — verdict: impl-wins — **The reconciler is called from `lib.rs::setup_server` as
  well as `main.rs`** (the plan said `main.rs` only). Reason: the module lives in the LIB, and the
  lib never calling it makes every one of its items dead code (`cargo check` emits `never used` for
  `reconcile`, `DesiredState`, `Mode`, …). Wiring the same one-line call into `setup_server` (the
  desktop/embedded boot path) removes the dead-code warnings AND makes the two boot paths behave
  identically. Behavior is unchanged for desktop: the call is gated on `ZIEE_DESIRED_STATE_FILE`,
  which the desktop app never sets, so it remains a no-op there (asserted by
  `test_no_env_var_means_no_reconcile`). PLAN.md ITEM-8 + "Files to touch" amended to include
  `src-app/server/src/lib.rs`.

- **DRIFT-1.2** — verdict: impl-wins — **`GroupEntry` has no `mode` field.** The plan gave every
  entry a `mode`, but a permission SET has no create-vs-update distinction: `ensure` and `enforce`
  would do the identical thing (idempotent set arithmetic), so a `mode` knob on a group would be a
  fake choice that implies a difference the code cannot honor. Group permissions are therefore
  always reconciled (which is also what makes a future `grant_*_to_users` migration self-
  correcting). `mode` remains meaningful — and implemented — for `mcp_servers` (create vs re-sync),
  and is accepted-but-inert for `admin`/`users` (both are pure "create if absent"; the plan's
  never-reset-the-password rule IS the ensure contract). DECISIONS.md DEC-5 amended to say so.

- **DRIFT-1.3** — verdict: impl-wins — **TEST-17 (the shipped `config/desired-state.yaml` is valid)
  is a `tier: unit` test in `modules/desired_state/mod.rs`, not an integration test.** It only needs
  to parse + validate the committed file — no server, no DB. Putting it in the unit tier makes it
  run in `cargo test --lib` (seconds) rather than requiring the whole integration harness. TESTS.md
  amended (tier + file).

- **DRIFT-1.4** — verdict: impl-wins — **ITEM-10's blast radius is larger than the plan's audit
  found.** PLAN/PLAN_AUDIT claimed "a ≥4 count assert + two `filesystem` lookups" in
  `tests/mcp/mod.rs` plus two e2e specs. The real dependency set (found by grepping AFTER the
  deletion) also includes:
  - a group-CASCADE test (`tests/mcp/mod.rs::test_system_server_assignment_cascades_to_group_members`)
    that needed a system server which is NOT already group-assigned. `fetch` ships assigned to the
    default group, so retargeting it there would have made the "before assignment" half vacuous.
    Fixed by giving that test its own `cascade_target` fixture row instead of leaning on a seed.
  - the e2e admin-server specs use `filesystem` as *the disabled* system server (vs the enabled
    `Web Fetch`) across FIVE tests (toggle, search, filter-enabled, filter-disabled, list). After the
    deletion no seeded DISABLED system server exists, so a plain retarget onto `fetch` was impossible.
    Fixed by seeding a `Disabled Fixture` system server via the admin API in the spec's `beforeEach`
    (the e2e harness gives each test its own database, so the fixture is fresh per test).
  - a stale comment block in `tests/mcp/mod.rs` naming the four admin-configurable built-ins.
  No plan item changed — the ITEM-10 scope statement in PLAN.md is amended to name these.

- **DRIFT-1.5** — verdict: resolved — **`usage_mode: auto` is now pinned explicitly on BOTH the
  create and the enforce/update path** (and asserted in TEST-5). Human instruction received during
  implementation: "for the MCP servers we want to add, please make sure that their modes are all
  auto (let the LLM decide)". The create path already sent `Auto`; the update path sent `None`
  ("don't touch"), which would have let an `enforce` re-sync silently preserve a drifted
  `usage_mode`. Recorded as FB-1 in HUMAN_FEEDBACK.md.

- **DRIFT-1.6** — verdict: resolved — the plan's "no test-harness change" and "no product UI code
  change" constraints HELD. `tests/common/harness_inner.rs` is untouched (the new tests drive the
  reconciler purely through `TestServerOptions::extra_env`, which already existed), and no file
  under `src-app/ui/src/**` is modified — the frontend diff is e2e specs only.

**Unresolved drifts:** 0
