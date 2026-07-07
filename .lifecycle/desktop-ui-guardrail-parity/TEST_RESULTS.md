# TEST_RESULTS — desktop-ui-guardrail-parity

Diff touches ONE frontend workspace (`src-app/desktop/ui`); no backend paths.
So the frontend gate chain applies to `desktop/ui` only.

## Frontend gate

- npm run check (desktop/ui): PASS

`cd src-app/desktop/ui && npm run check` → exit 0 (tsc + biome guardrails +
lint:colors + lint:settings-field + **lint:adjacent-inline + lint:icon-action** +
lint:logical-direction + lint:tooltip-placement + **check:kit-manifest +
check:testid-registry + check:design-spec** + check:gallery-coverage +
check:gallery-crawl + gallery:check-fixtures + check:state-matrix +
**check:overlay-registry**). The bold gates are the backfilled ones.

UI evaluator gate (`npm run gate:ui`) → GATE PASSED: tsc PASS, lint PASS,
runtime-health 41/41 surfaces clean (HIGH 0 gating), coverage in sync.

## Per-TEST results

- **TEST-1**: PASS   (vitest — check chain includes the 6 backfilled gates; ../../ui/scripts refs resolve)
- **TEST-2**: PASS   (vitest — gallery-geometry-audit.mjs byte-identical to web; gallery:geometry(:gate) defined; geometry-allowlist parses)
- **TEST-3**: PASS   (vitest — affordance-audit.mjs + allowlist exist; gallery:affordance defined)
- **TEST-4**: PASS   (vitest — detector-acceptance.mjs exits 0: C11/J8 lint detectors FIRE + geometry byte-identity OK)
- **TEST-5**: PASS   (vitest — gen-crop-review + docs/DEFECT_TAXONOMY.md present; taxonomy carries [V] rubric)
- **TEST-6**: PASS   (vitest — overlays.tsx + allowlist + generated registry; check:overlay-registry --check exits 0)
- **TEST-7**: PASS   (`npm run check` exit 0 — the composite static gate; see frontend gate line above)
- **TEST-8**: PASS   (playwright gallery — settings-about/remote-access/host-mount render, no gallery-crash, light+dark)
- **TEST-9**: PASS   (playwright gallery — 0 console/page error in loaded state, light+dark)
- **TEST-10**: PASS  (playwright gallery — geometry --gate on the 3 desktop-only surfaces exits 0)
- **TEST-11**: PASS  (playwright gallery — affordance gate runs green on desktop surfaces)
- **TEST-12**: PASS  (playwright gallery — axe a11y: no serious/critical violation, light+dark)

## Commands

```
cd src-app/desktop/ui
npx vitest run --config vitest.config.ts src/dev/guardrails/     # TEST-1..6 → 21 passed
npm run check                                                    # TEST-7 → exit 0
npx playwright test -c playwright.gallery.config.ts              # TEST-8..12 → 8 passed
npm run gate:ui                                                  # evaluator gate → PASSED
```

All Phase-3 tests PASS. No `#[ignore]`/`.skip` used to go green.
