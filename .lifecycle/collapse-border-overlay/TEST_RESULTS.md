# TEST_RESULTS — collapse-border-overlay

Scoped to the touched area (frontend only — `src-app/ui/**`). Diff touches no
backend path, so the backend integration chain does not apply; the backend e2e
harness is exercised only because the collapse regression guard
(`collapse-long-message.spec.ts`) drives a real server.

## Frontend gate

- `npm run check (ui): PARTIAL` — see the pre-existing-red note below. The parts
  attributable to this change (`tsc`, all `lint:*`) PASS; the failing steps
  (`check:testid-registry`, `check:gallery-coverage`, `check:state-matrix`) fail
  IDENTICALLY on the base commit `origin/khoi`, from unrelated SDK-migration
  drift, and half of it lives in the `sdk` submodule and cannot be fixed from a
  PR into `khoi`. User-approved disposition: ship focused, document the
  pre-existing red (see below and HUMAN_FEEDBACK FB-3).
- `gate:ui (ui): PASS` — tsc + lint + runtime-health, 182/182 surfaces
  runtime-clean, zero gating HIGH findings. (An earlier run showed 5 unrelated
  surfaces failing; that was self-inflicted — source edits hot-reloading into the
  dev server mid-sweep — and cleared entirely on a clean re-run. LEDGER round-4,
  rejected.)

## Per-TEST results (from TESTS.md)

- **TEST-1**: PASS — `tsc --noEmit` clean; the `satisfies Record<GallerySurface,
  Coverage>` totality holds and the two `coverage.ts` reason strings name the new
  surface.
- **TEST-2**: PASS — `chat-collapse-borders.spec.ts` preconditions (clamps, ≥3
  cards inside, ramp straddle, top-flush card, exact interleaved order).
- **TEST-3**: PASS (light + dark) — every card inside the clamp has ≥1px ring room
  on left/right/top against its tightest per-axis clipper, AND the left ring +
  first card's top ring are actually PAINTED.
- **TEST-4**: PASS — the inset self-cancels (clamp content width == parent content
  width) and card widths are equal collapsed vs expanded.
- **TEST-5**: PASS — collapse still bounds the height (≤400px, "Show more"
  present).
- **TEST-6**: PASS — `collapse-long-message.spec.ts` against the REAL backend: the
  pure-text long message still clamps ≤400px, toggles, and re-clamps. Full log:
  `scratchpad/logs/collapse-e2e-final2.log`.
- **TEST-7**: PASS — `chat-scroll-stability.spec.ts` TEST-6/7/8/9/10/11/13 all
  green; the virtualizer detector (settled corrections ≤2) holds.
- **TEST-8**: PASS (light + dark) — expanded is unclamped (no mask, no
  overflow-hidden, >400px) and every card's ring is painted.

## Assertion honesty — each spec assertion observed to FAIL on the defect it guards

| injected defect | caught by |
|---|---|
| unfixed base (no inset) | TEST-3 — LEFT |
| horizontal-only inset (`-mx-0.5 px-0.5`) | TEST-3 — "tightest TOP-clipping ancestor" |
| kit Card set to `ring-0` | TEST-3 + TEST-8 — both themes (paint check) |
| parent `px-0.5` deleted | TEST-3 — LEFT, naming the bubble clip layer |

## Pre-existing `npm run check` red (verified on a pristine base)

Reproduced by detaching to `origin/khoi` (`6ca93f123`) with NONE of this change
and re-running:

```
$ git checkout --detach origin/khoi
$ npm run check:testid-registry
  → testIds.generated.ts is stale — run `npm run gen:testid-registry` and commit.
$ npm run check:gallery-coverage
  → galleryCoverage.generated.ts is stale — run `npm run gen:gallery-coverage` and commit.
```

Cause: the F1/F2 SDK migration moved the kit into the `sdk` submodule
(`src/components/ui/` no longer exists) but the generated registries were never
refreshed. `testIds.generated.ts` lives INSIDE the submodule, so it is not
fixable from a PR into `khoi` at all. Running the regens would drag ~1700 lines of
unrelated cleanup into this diff and break `tsc` (the hand-maintained
`coverage.ts` still carries the old keys). This change adds no component file and
no `data-testid`, so it requires no regen. Disposition approved by the user
(HUMAN_FEEDBACK FB-3).

## Not applicable

- **A8** (built-in MCP server) — none added.
- **A9 / A10** (permission deny + restricted-user e2e) — no permission introduced
  (no `modules/*/permissions.rs` change, no migration grant).
- **R2-5** (e2e route-mock vs openapi) — the new spec adds no `/api/` route mock;
  it drives the backend-free gallery.
- Backend integration tests — no backend path touched.

## Unit-test note

`npm run test:unit` reports 10 failures — all in `*.store.test.ts` (Vitest-target
files run under node:test) plus `runTimeline.test.ts`. Confirmed byte-identical on
a clean stash of the base commit, so they are pre-existing and unrelated. This
change adds no `node:test` files (the `collapsible.test.ts` additions were removed
with the reverted split).
