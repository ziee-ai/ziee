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

Totals: 30 unit assertions green (8 core node:test + 10 codemod/manifest node:test + 12 desktop vitest).

## E2E + runtime gate

- **TEST-9** (desktop e2e — relocated `.desktop.tsx` + seam variants render in the real desktop build): PENDING
- **TEST-10** (web e2e — seam fallback / no `.desktop` leakage): PENDING
- **TEST-11** (desktop gallery runtime — zero console/contrast errors): PENDING
- `gate:ui` (runtime-health + Layer A/axe): PENDING

E2E requires a booted app (Playwright + docker/backend); the green desktop
`vite build` + `npm run check` (which includes the gallery runtime-adjacent
static gates) are the strongest signals obtained so far. E2E to be run as the
final layer.
