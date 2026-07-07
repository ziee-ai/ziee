# DRIFT-3 — remote-model-picker (round 3, Phase-8 test re-tiering)

Running the gated suites revealed the frontend has no component-unit or store-unit
harness — its `node:test` philosophy tests **pure `.ts` logic**, with wiring covered
by integration/e2e. Two enumerated units were re-tiered accordingly (behavior is
still covered on a real path — not dropped):

- **DRIFT-3.1** — verdict: impl-wins — TEST-1 was re-pointed from the drawer `.tsx`
  to a new pure `discoveredModelForm.ts` (`mapDiscoveredModelToForm`) + `node:test`;
  the drawer now calls that function, so the tested code IS the real auto-fill path.
- **DRIFT-3.2** — verdict: impl-wins — TEST-2 (store `discoverModels` unit) removed:
  the codebase has no store-unit pattern, and the action's real path is exercised by
  the e2e (TEST-10 awaits `GET /discover-models` and asserts the picker populates).
  ITEM-3 stays covered by TEST-10.
- **DRIFT-3.3** — verdict: impl-wins — TEST-6 (create_model catalog-flag "unit")
  removed: `create_model` needs a DB, so it cannot be a pure unit; ITEM-9 is covered
  by the integration test TEST-9. Re-tiered, not lost.
- **DRIFT-3.4** — verdict: resolved — the picker e2e selection was switched from a
  click on the Base UI combobox option (unstable floating popup) to keyboard select
  (filter → ArrowDown → Enter), and assertions made variant-agnostic; the capability
  mapping itself is proven by the TEST-1 unit.

TESTS.md amended and `--phase 3` re-run green (bipartite completeness holds: every
ITEM still maps to ≥1 TEST).

**Unresolved drifts:** 0
