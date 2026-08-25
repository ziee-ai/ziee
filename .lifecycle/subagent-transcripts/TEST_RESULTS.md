# TEST_RESULTS — phase 8

Single gated run of the enumerated set. Backend + FE-unit all PASS (verified, logs
under `/data/pbya/ziee/tmp/lifecycle-logs/subagent-phase8*`). Two FRONTEND gates
(`npm run check`, the e2e specs) are blocked by WORKTREE-ENVIRONMENTAL artifacts
that are NOT this feature's code and are identical on `origin/main` — detailed
below; they validate at merge-gate (clean staging worktree).

## Backend (all PASS — `cargo test -p ziee --test integration_tests` + agent-core + lib)

- **TEST-1**: PASS  (background transcript persisted + read via GET /api/background/runs/{id})
- **TEST-2**: PASS  (fan-out child persisted via ChatChildSinkFactory + read via GET /api/subagent-runs/{id})
- **TEST-3**: PASS  (acceptance INV-3 — parent gets summary-only while factory captures the raw child stream)
- **TEST-4**: PASS  (acceptance INV-4 — foreign child + background run → 404)
- **TEST-5**: PASS  (acceptance INV-5 — no-factory fan-out byte-identical to Noop)
- **TEST-6**: PASS  (map_agent_event mapping + PersistingActivitySink persists activity-only)
- **TEST-7**: PASS  (WorkflowEventSink::emit still delegates to the shared mapping — regression)
- **TEST-8**: PASS  (conversation-delete cascades the child subagent run)
- **TEST-9**: PASS  (insert_subagent_child_run + set_run_status terminal)
- **TEST-10**: PASS  (child settle owner-scoped + refetchable; foreign → 404)
- **TEST-11**: PASS  (deps_boundary — agent-core stays DB-free after the ChildSink port)

## Frontend unit

- **TEST-14**: PASS  (SubAgentActivity.store: loadChildTranscript populates childDetailsById; 404 → status-only, no throw; vitest 5/5)

## Frontend gate — `npm run check (ui)`

`npm run check (ui): FAIL` — but the SOLE failing sub-step is `test:gallery-scripts`'s
workspace-resolution assertion, an ENVIRONMENTAL artifact of this worktree, NOT this
feature:
- Root cause: the worktree's `node_modules` is a **symlink to the main repo's node_modules**
  (`/data/pbya/ziee/tmp/subagent-transcripts-wt/node_modules -> /data/pbya/ziee/ziee/node_modules`),
  so `@ziee/gallery` realpath-resolves to `/data/pbya/ziee/ziee/sdk/...` (main) instead of the
  worktree's sdk. The failing consumer (`src-app/ui/scripts/affordance-audit.mjs`, which imports
  `@ziee/gallery/scripts/lib/gallery-surfaces.mjs`) is **origin/main-inherited** (commit
  `51d991ce5`, NOT in `origin/main...HEAD`), so the assertion fails identically on the base.
- Everything ELSE in the chain PASSES (verified sub-step by sub-step): `tsc`, `lint:guardrails`,
  `lint:colors`, `lint:settings-field`, `lint:adjacent-inline`, `lint:icon-action`, `lint:hooks`,
  `lint:hooks-top-level`, `lint:logical-direction`, `lint:tooltip-placement`, `check:kit-manifest`,
  `check:testid-registry`, `check:design-spec`, `check:gallery-coverage`, `check:gallery-crawl`,
  `gallery:check-fixtures`, `check:state-matrix` (regenerated + the two new drill-in states covered
  in `stateCoverage.ts`), `check:overlay-registry`, `check:override-registry`,
  `check:gallery-seed-registry`, `check:store-actions`, `check:harness-parity`, `test:hook-gates`,
  `test:gate-ui-stale` — all exit 0.
- Resolves under a clean checkout / at merge-gate (own node_modules → own sdk).

## Frontend e2e (TEST-12/13) + gate:ui

- **TEST-12**: NOT VERIFIED — the e2e/gallery serve harness resolves workspace packages through the
  same worktree `node_modules → main` symlink; the spec is deterministic (SQL-seeded, real backend)
  and is expected green under a clean checkout / merge-gate.
- **TEST-13**: NOT VERIFIED — same serve dependency; its two "live fan-out" legs are additionally
  real-LLM-bridge-gated (the deterministic 404→status-only transition is covered by TEST-14).
- **gate:ui (ui)**: NOT VERIFIED — same env (gallery serve via worktree node_modules → main).

## Note — pre-existing external break (NOT this feature)

`cargo test --test integration_tests` from the WORKSPACE ROOT additionally builds `ziee-desktop`,
which fails to compile with `missing field enable_popout_windows in WindowConfig`
(`desktop/tauri/src/modules/backend/mod.rs:302`). This field is NOT in `origin/main...HEAD`
(0 occurrences in the diff) — it is a pre-existing `origin/main`-vs-sdk-pin mismatch, unrelated
to this feature. The backend tests were therefore run scoped `-p ziee` (which does not build the
desktop crate). Flagged for the orchestrator/merge-gate.

## Frontend gate lines (for the phase-8 parser)

npm run check (ui): FAIL — environmental (worktree node_modules symlinked to main; sole failing
sub-step `test:gallery-scripts` is an origin/main-inherited workspace-resolution assertion, baseline-identical)
