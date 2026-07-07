# TEST_RESULTS — kit-table-actions

Diff touches only `src-app/ui/**` (pure frontend). Backend chain N/A. Frontend
chain run in the `ui` workspace.

## Frontend gate

`npm run check (ui): PASS`

(tsc + biome guardrails + lint:colors + lint:settings-field + lint:icon-action +
lint:logical-direction + lint:tooltip-placement + check:kit-manifest +
check:testid-registry + check:design-spec + check:gallery-coverage +
check:gallery-crawl + gallery:check-fixtures + check:state-matrix +
check:overlay-registry — all green.)

## Unit (`node --test`, 26 tests green)

- **TEST-1**: PASS
- **TEST-2**: PASS
- **TEST-3**: PASS
- **TEST-4**: PASS
- **TEST-5**: PASS
- **TEST-6**: PASS
- **TEST-7**: PASS
- **TEST-8**: PASS
- **TEST-9**: PASS
- **TEST-10**: PASS
- **TEST-11**: PASS

## e2e (Playwright, gallery config, 17 tests green — `--workers=1`)

- **TEST-12**: PASS
- **TEST-13**: PASS
- **TEST-14**: PASS
- **TEST-15**: PASS
- **TEST-16**: PASS
- **TEST-17**: PASS
- **TEST-18**: PASS
- **TEST-19**: PASS
- **TEST-20**: PASS
- **TEST-21**: PASS
- **TEST-22**: PASS
- **TEST-23**: PASS
- **TEST-24**: PASS
- **TEST-25**: PASS
- **TEST-26**: PASS
- **TEST-27**: PASS
- **TEST-28**: PASS

Run command:
```
GALLERY_PORT=1494 npx playwright test -c playwright.visual.config.ts \
  tests/e2e/visual/kit-table-capabilities.spec.ts \
  tests/e2e/visual/tabular-viewer.spec.ts \
  tests/e2e/visual/data-grids.spec.ts --project=gallery
→ 17 passed
```
