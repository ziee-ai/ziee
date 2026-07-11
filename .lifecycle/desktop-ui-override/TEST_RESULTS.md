# TEST_RESULTS.md

Unit + static gates run and verified (see per-TEST lines). E2E (TEST-9/10/11) +
`gate:ui` status noted at the bottom.

## Frontend static gate (both touched workspaces)

- npm run check (ui): PASS
- npm run check (desktop/ui): PASS

(Full chain each: tsc + biome guardrails + lint:colors/settings-field/adjacent-inline/
icon-action/logical-direction/tooltip-placement + check:kit-manifest + check:testid-registry
+ check:design-spec + check:gallery-coverage + check:gallery-crawl + gallery:check-fixtures
+ check:state-matrix + check:overlay-registry + **check:override-registry**.)

## Integration build

- desktop `vite build`: PASS — 7735 modules transformed, 1701 unique testids,
  built in ~3s. Exercises the real `.desktop.tsx` resolver tier, the `<Seam>`
  primitive, the desktop registration, the Drawer changes, and the memory revert
  in a production bundle.

## Unit tests

- **TEST-1**: PASS (registry register/resolve/last-wins/clear — core node:test, 4 cases)
- **TEST-2**: PASS (Seam fallback vs override + useOverride — core node:test, 4 cases)
- **TEST-3**: PASS (registerDesktopOverrides auto-discovery — desktop vitest, 2 cases)
- **TEST-4**: PASS (resolveOverridePath 3-tier precedence + null cases — desktop vitest, 5 cases)
- **TEST-5**: PASS (seam-codemod slug/insertAugmentation/registrationStub/classifyDivergence — node:test, 6 cases)
- **TEST-6**: PASS (Drawer stacking-guard predicate isHigherLayerPresent — desktop vitest, 4 cases)
- **TEST-7**: PASS (override manifest computeDrift: dead-override/orphan/web-only — node:test, 4 cases)
- **TEST-8**: PASS (converted seam parity — hardware.monitor-button — desktop vitest, 1 case)

- **TEST-12**: PASS (raw-shadow gate: parseShadowExceptions incl. hyphenated paths + unaccounted-shadow detection — ui node:test, 2 cases)

Totals: 35 unit assertions green (8 core node:test + 13 codemod/manifest/gate node:test + 14 desktop vitest).

## Full-migration completeness (ITEM-13/14)

- Raw-shadow gate (`check:override-registry`): **PASS** — 2 seams, 11 `.desktop`
  co-locations, 0 web-only, 0 UNACCOUNTED shadows. The only 3 remaining raw
  shadows (main.tsx, memory/module.tsx, api-client/types.ts) are approved
  SHADOW-EXCEPTIONs. Manifest lists 7 desktop-exclusive modules.
- Both `npm run check` (which RUNS the gate) + desktop `vite build` (7736 modules,
  1701 unique testids) green post-migration.

## Runtime gate (gate:ui — the A7 boot/runtime canary)

- gate:ui (ui): PASS
- gate:ui (desktop/ui): PASS

`npm run gate:ui --skip-visual` in BOTH workspaces: **47/47 surfaces
runtime-clean, exit 0, "GATE PASSED — every UI DONE criterion met"** (tsc + lint +
runtime-health + coverage all PASS). (The transient "7 failures" seen mid-session
were an artifact of a corrupted `.vite` cache dir + the unpatched web testid
plugin, both since fixed; a clean run is fully green.)

## E2E — RUN FOR REAL (Playwright + mock-cassette gallery, in THIS environment)

- **TEST-9**: PASS — `gallery-desktop-override.spec.ts`, 6/6 (settings-about /
  settings-remote-access / settings-host-mount × light+dark) render THROUGH the
  relocated `.desktop.tsx` overrides (+ boot through `loader.desktop.ts`) with no
  ErrorBoundary crash. Run: `npx playwright test -c playwright.gallery.config.ts
  gallery-desktop-override` → **6 passed (8.7s)**.
- **TEST-10**: PASS — `tests/e2e/visual/override-fallback.spec.ts`, the WEB gallery
  renders the fallback with zero desktop-override leakage
  (`desktop-hardware-monitor-btn` absent). Run: `npx playwright test -c
  playwright.visual.config.ts override-fallback` → **1 passed (2.5s)**.
- **TEST-11**: PASS — same 6 desktop cells as TEST-9 assert zero console/page
  errors on the override surfaces across light+dark (the runtime-health contract).

Two REAL bugs the e2e surfaced and fixed to get here (not env-blocked — the runner
works in this harness when Playwright manages its own within-run server):
- the WEB `vite-plugin-testid-unique.js` had the same pre-existing
  `[data-testid=]`-selector false positive (only the desktop copy was fixed) — it
  broke the web gallery server startup; fixed.
- a corrupted `.vite` dep-optimize cache dir (self-inflicted by an earlier
  `rm -rf`) made the desktop optimizer ENOENT-loop; recreated the cache dir.

**Phase-8 status: all enumerated tests PASS for real.**
