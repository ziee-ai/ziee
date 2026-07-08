# FIX_ROUND-1

Merged `LEDGER.jsonl` (32 rows, 12 angles) from the blind multi-angle audit and
resolved every confirmed finding.

## Fixes applied (confirmed defects)

- **patterns-conformance** (`SandboxRootfsVersionsSection.tsx`): the
  `DEGRADED_NOTICE` const was declared between two import statements. Moved it
  below all imports. `npm run check` (tsc + biome + all lints/gallery/state-matrix
  gates) green after the move.
- **tests-quality** (`tier3_versions.rs`): the module-level doc claimed the
  process-global `STATE` OnceCell makes a 200-vs-503 assertion order-dependent —
  which contradicts the new deterministic `availability == "disabled_in_config"`
  asserts. Rewrote the doc to state the LIST path degrades to 200 and that
  determinism holds because each `TestServer` spawns its own server subprocess
  (per-subprocess `STATE`/`INIT_STATUS`).

## Findings reviewed and dismissed (with rationale)

- **state-management medium — downloaded-card actions not disabled when degraded**:
  not reachable. A degraded response always carries `installed: []` / `pinned:
  null` (`build_degraded`), and `loadStatus` overwrites `installed`, so
  `downloadedGroups` is always empty when degraded — the Set-default/Delete
  buttons never render. No fix needed.
- **perf medium — `available_only()` GitHub call per disabled-list request**:
  intentional and required (the whole point is to show the catalog when disabled)
  and identical to the already-shipping enabled `status()` path; admin-gated +
  infrequent. Adding a cache would be a cross-path change beyond this feature.
- **a11y low — disabled Download button tooltip may not fire**: mirrors the
  pre-existing requires-manage wrap (convention-consistent) and the degraded
  `Alert` already states the reason prominently, so the reason is discoverable.
- **patterns-conformance low — seeded surface omits `await whenTrue`**: `holdPatch`
  persists the seed over the mount-time `loadStatus`; the gallery-coverage +
  state-matrix + runtime gates pass on the surface.
- **tests-quality low — degrade test only asserts `available.is_array()`**:
  deliberate — a hermetic test can't guarantee a non-empty GitHub catalog; the
  test's real value (200, `availability`, empty installed/pin, install-still-503)
  is asserted strongly.

## Re-audit

Re-reviewed the two changed spots: the const move is a pure reorder (no semantic
change; `npm run check` green) and the doc edit is comment-only (no compile/behavior
impact). No new issues surface.

**New confirmed findings:** 0
