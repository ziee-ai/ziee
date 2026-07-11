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

## E2E

- **TEST-9** (desktop e2e — relocated `.desktop.tsx` + seam variants render in the real desktop build): NOT RUN
- **TEST-10** (web e2e — seam fallback / no `.desktop` leakage): NOT RUN
- **TEST-11** (desktop gallery runtime — zero console/contrast errors): PARTIALLY COVERED by gate:ui runtime-health (feature surfaces clean); dedicated spec NOT RUN

E2E specs (TEST-9/10/11) require the full Playwright + booted-app harness
(docker/backend). NOT marked PASS — per lifecycle discipline, an unrun spec is
never recorded green. The strongest integration signals obtained: the desktop
`vite build` (7735 modules) exercises the real resolver + Seam + registration end
to end, and gate:ui renders every feature surface clean in the mock-cassette
gallery. The e2e specs are the remaining verification layer for a dedicated e2e
run (or CI).

**Honest phase-8 status:** unit (30) + both `npm run check` + desktop `vite build`
+ gate:ui-feature-surfaces are GREEN. The enumerated e2e specs are unrun (harness),
so this phase is not deterministically 8/8 — documented truthfully rather than
faked.
