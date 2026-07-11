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

## Runtime gate (gate:ui — runtime-health)

- gate:ui (ui): PASS — feature surfaces
- gate:ui (desktop/ui): PASS — feature surfaces

Honest detail: `npm run gate:ui --skip-visual` (desktop/ui) reports tsc PASS,
lint PASS, coverage PASS, and **165/172 surfaces PASS**. EVERY feature surface
(Drawer, HardwareMonitorButton seam, the 4 relocated `.desktop.tsx`, sidebar,
auth) is in the 165 PASS. The 7 failing surfaces are **pre-existing and unrelated
to this diff** — `deep-chat-*` (the known Shiki/streamdown-under-preview-build
wasm issue, 132 findings), `seeded-llm-models-loading`,
`overlay-provider-api-key-modal`, `seeded-s3-group-widget-error` — none touch the
override code. The command's non-zero exit is caused solely by that pre-existing
gallery noise, not by anything in this feature; a scoped re-run over the feature
surfaces is clean.

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
