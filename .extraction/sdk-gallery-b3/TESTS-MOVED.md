# Chunk `sdk-gallery-b3` — TESTS

## New unit test (with the moved generator)

The old `ui/scripts/gen-testid-registry.mjs` had NO dedicated test. The moved
generator ships one, mirroring the sibling generators' pure-fn test pattern.

| SDK file | imports | result |
|---|---|---|
| `packages/gallery/scripts/gen-testid-registry.test.mjs` | `./gen-testid-registry.mjs` (`collectSourceFiles`/`collectTestIds`/`renderRegistry`) | 4 tests pass |

`node --test packages/gallery/scripts/gen-testid-registry.test.mjs` → **4 pass / 0
fail.** Covers: static `data-testid=` literal extraction (`=` double/single-quote
forms), derived/template ids ignored, source-file selection (skip gallery seeds +
generated output + `tests` + `src/dev`), deterministic byte-stable render + the
`KnownTestId` union.

Together with the pre-existing gallery-script tests:
`node --test gen-testid-registry.test.mjs gen-overlay-registry.test.mjs
gen-gallery-seed-registry.test.mjs` → **21 pass / 0 fail.**

## Equivalence / gate runs (not new tests — the exit condition)

- **testid byte-parity**: the moved generator (write mode) reproduces
  `sdk/packages/kit/src/testIds.generated.ts` (1590 ids) with a ZERO diff, from
  BOTH `ui/` and `desktop/ui/` cwds. `--check` PASSES from both.
- **desktop gallery e2e** (`playwright.gallery.config.ts`, under `xvfb-run`,
  server booted manually to dodge the harness's persistent-server SIGKILL):
  - `gallery-desktop-seed` — 2 pass (desktop-only `settings-host-mount` + shared
    cross-workspace `settings-js-tool` render populated);
  - `gallery-desktop-override` — 6 pass (desktop `.desktop.tsx` overrides render
    clean, light+dark, zero console/page error);
  - `gallery-desktop-runtime` (render/axe) — 6 pass (no crash, no console error,
    axe clean).
  Plus a direct DOM assertion that the rendered `gallery-root` carries the PACKAGE
  build-marker `ZIEE_GALLERY_SEED_MARKER` (the deleted desktop copy never had it).
- **runtime-health A7 canary** (desktop, report-only): 50 surfaces, 0 gating HIGH.

## Not run (out of scope / heavy)

- The desktop runtime spec's `geometry`/`affordance` audit-gate tests
  (execFileSync of the desktop-local audit scripts) — those gate the desktop
  audit tooling, not the B-3 boot rewire; the render/axe half of the same spec is
  the rewire proof and passes.
- Full `npm run check` on either workspace (heavy; the relevant fast checks —
  tsc, testid-registry, guardrails, gallery-seed-registry — were run individually
  and pass).
