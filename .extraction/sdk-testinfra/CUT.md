# Chunk `sdk-testinfra` — the Layer-B test scaffold + gallery follow-ups (CUT manifest)

Two deliverables, both additive / backward-compat:

1. **`@ziee/test-e2e` (CUT — ADDITIVE).** A NEW package holding the GENERIC
   Playwright Layer-B scaffold distilled from ziee's `src-app/ui/tests/e2e/`
   (`global-setup.ts` docker-DB bring-up + server/UI warmup, `global-teardown.ts`,
   the `testid.ts` selectors, and the base `playwright.config` presets),
   parameterized over one `E2EConfig` seam (server command / base URL / test dir /
   docker template / ui-build / warmup / first-run hook). **ziee's own e2e stays
   app-side, byte-unchanged** — this is a scaffold a FRESH app extends, not a
   re-home of ziee's specs. Server + desktop variants both ship.

2. **`@ziee/gallery` B1/B2/B4 (MOVE).** Re-home the remaining config-drivable
   gallery scripts under `@ziee/gallery/scripts/*` + `@ziee/gallery/playwright/*`,
   parameterized over the SAME `gallery.config.json` the runner + gates already
   read, and repoint ziee's npm scripts + `vite.config.ts` plugins at them.

## Deliverable 1 — `@ziee/test-e2e` (new files, no source-deletion)

Additive: nothing is removed from ziee (its e2e keeps working identically). The
package supplies a reusable, DI'd scaffold.

| SDK file | distilled from (ziee, app-side, UNCHANGED) |
|---|---|
| `packages/test-e2e/src/config.ts` (`E2EConfig`, `defineE2EConfig`) | the union of hardcodes across ziee's `playwright.config.ts` + `tests/global-setup.ts` |
| `packages/test-e2e/src/global-setup.ts` (`createGlobalSetup`) | `ui/tests/global-setup.ts:15-305` (docker reap + PG bring-up + UI build + server warmup) |
| `packages/test-e2e/src/global-teardown.ts` (`createGlobalTeardown`) | `ui/tests/global-teardown.ts:9-61` |
| `packages/test-e2e/src/port-manager.ts` (lean generic allocator) | `ui/tests/fixtures/port-manager.ts` (the shared-lock `{pid,runId}` liveness core) |
| `packages/test-e2e/src/playwright-preset.ts` (`definePlaywrightPreset` / `defineDesktopPreset`) | `ui/playwright.config.ts:15-136` + `desktop/ui/playwright.config.ts:14-89` |
| `packages/test-e2e/src/testid.ts` (`byTestId`/`makeByTestId`/`byRole`/`byLabel`/`byText`) | `ui/tests/e2e/testid.ts:14-15` |
| `packages/test-e2e/src/index.ts` | barrel |

## Deliverable 2 — B1/B2 MOVE table (source→dest, generic engine leaves ziee)

MOVE = `git rm` from ziee `src-app/ui/scripts` (+ `plugins/`) → new file in the
package; ziee's npm scripts repointed (B4). Every moved generator anchors on
`resolveGalleryConfig(process.cwd())` instead of the old `HERE`-relative
hardcodes, so the app's `ui/` cwd + `gallery.config.json` drive every path.

| ziee source (DELETED) | SDK dest | anchor change |
|---|---|---|
| `ui/scripts/gen-state-matrix.mjs:48-53,247,472` | `packages/gallery/scripts/gen-state-matrix.mjs` | `HERE/UI_DIR/SRC` → `CFG.__cwd`+`srcDir`+`galleryDir`+`surfaceRoots`+`tsconfig` |
| `ui/scripts/gen-overlay-registry.mjs:38-64,187` | `.../gen-overlay-registry.mjs` | `SRC/GALLERY/ROOTS` + `@/components/ui` regex → `srcDir`/`galleryDir`/`surfaceRoots`/`overlayKitImports` |
| `ui/scripts/gen-overlay-registry.test.mjs` | `.../gen-overlay-registry.test.mjs` | pure-fn import (unchanged) |
| `ui/scripts/gen-gallery-coverage.mjs:23-32` | `.../gen-gallery-coverage.mjs` | `SRC/OUT/COVERAGE/ROOTS` → `srcDir`/`galleryDir`/`surfaceRoots` |
| `ui/scripts/gen-gallery-seed-registry.mjs:25-35` | `.../gen-gallery-seed-registry.mjs` | `HERE/../src` default → `CFG.__cwd`+`srcDir`; `--src` override preserved |
| `ui/scripts/gen-gallery-seed-registry.test.mjs` | `.../gen-gallery-seed-registry.test.mjs` | pure-fn import (unchanged) |
| `ui/scripts/gallery-coverage.mjs:38-44` | `.../gallery-coverage.mjs` | `HERE/UI_DIR/SRC/GALLERY` → `srcDir`/`galleryDir` |
| `ui/scripts/check-gallery-prod-exclusion.mjs:19-23,31` | `.../check-gallery-prod-exclusion.mjs` | `DIST`/`MARKER`/build-cmd → `distDir`/`prodMarker`/`buildCmd` |
| `ui/scripts/capture-gallery-screenshots.mjs` | `.../capture-gallery-screenshots.mjs` | verbatim (CLI `--url`/`--out`; imports package `lib/gallery-surfaces.mjs`) |
| `ui/scripts/capture-gallery-states.mjs` | `.../capture-gallery-states.mjs` | verbatim (CLI-driven) |
| `ui/plugins/vite-plugin-gallery-alias.js` | (already in `packages/gallery/vite/`) — DELETE ziee dup | byte-identical dup removed |
| `ui/plugins/vite-plugin-gallery-coverage.js` | (already in `packages/gallery/vite/`) — DELETE ziee dup | byte-identical dup removed |

New config fields added to `packages/gallery/scripts/lib/gallery-config.mjs`
(all default to ziee's historical hardcode → an app shipping no config behaves
exactly as before): **`srcDir`**, **`overlayKitImports`**, **`tsconfig`**
(`distDir`/`prodMarker`/`buildCmd`/`galleryDir`/`surfaceRoots` already existed).

## Deliverable 2 — B2 (playwright templates, config-driven, NEW in package)

- `packages/gallery/playwright/visual.config.ts` — `defineVisualConfig(overrides?)`,
  the config-driven form of ziee's `playwright.visual.config.ts` (reads
  `gallery.config.json` port/galleryUrl/visualTestDir/devCmd/maxDiffPixelRatio).
- `packages/gallery/playwright/templates/{_gallery,layout.spec,states.spec,overlays.spec,gallery.spec}.template.ts`
  — generic, surface-list-driven (`window.__GALLERY_LIST_ALL_SURFACES__`) Layer-A
  (layout+axe) / states / overlays / Layer-B spec STARTERS a fresh app copies.
  ziee's baseline-coupled specs (`_gallery.ts`, `layout-baseline`, `axe-baseline`)
  STAY app-side.

## Deliverable 2 — B4 repoint (ziee edits — the equivalence mechanism)

- `ui/package.json` — 14 scripts repointed `scripts/X.mjs` → `../../sdk/packages/gallery/scripts/X.mjs`
  (the 8 moved generators/captures/coverage/prod-exclusion + the 2 `test:` unit refs).
- `desktop/ui/package.json` — `gen/check:gallery-seed-registry` repointed to the
  package (`--src src` preserved; byte-identical from desktop cwd).
- `ui/vite.config.ts` — `galleryAliasPlugin`/`galleryCoveragePlugin` imports
  repointed `./plugins/*.js` → `@ziee/gallery/vite/*.js` + the two local dup
  plugin files `git rm`ed. Boot-smoke: gallery vite server serves `/gallery.html`
  + the alias-rewritten `/dev-gallery.html`.

## Design-gate — the moved generators produce BYTE-IDENTICAL output (TOP RISK)

Each moved generator was run BOTH ways (ziee-original vs package, `ui/` cwd) in
write mode and the emitted committed artifact diffed:
`stateMatrix.generated.ts` / `STATE_MATRIX.md` / `galleryCoverage.generated.ts` /
`overlay-registry.generated.json` — **all IDENTICAL**. `gen-gallery-seed-registry`
`--check` passes from BOTH `ui/` (39 modules) and `desktop/ui/ --src src` (9
modules). The 17 relocated unit tests pass (`node --test`). → repointing ziee's
`check:*` scripts is behavior-preserving. (Coverage/overlay/state-matrix committed
files were ALREADY stale on this branch — original + package both report the same
"stale", identically; the parity proof is the write-mode byte-diff, not the
pre-existing staleness.)

## Design-gate — `gen-testid-registry` CANNOT be cleanly parameterized → DEFERRED

The committed testid registry now lives at `sdk/packages/kit/src/testIds.generated.ts`
(1590 ids), and it includes ids from the KIT PACKAGE's own src (`app-root`,
`${testid}-row-${cssEscape(rk)}`). ziee's `ui/scripts/gen-testid-registry.mjs`
still writes to the DELETED path `src/components/ui/testIds.generated.ts` and walks
only `ui+desktop` (1588 ids) — it is mid kit-extraction migration and produces a
DIFFERENT set than the committed kit file. Reconciling that is the kit-migration's
job, not this chunk's. Per the "STOP if a generator can't be parameterized cleanly"
rule, `gen-testid-registry` is **left app-side, untouched** (ziee behaves
identically) and recorded as a follow-up in BOUNDARY. The other 4 generators are
clean and moved.

## Design-gate — `@ziee/test-e2e` is ADDITIVE (no ziee e2e repoint)

ziee's `playwright.config.ts` / `global-setup.ts` / `fixtures/*` / specs are
UNCHANGED — the package is a fresh-app scaffold, so there is NO equivalence-vs-ziee
requirement for it (nothing was cut FROM ziee). Its exit condition is
`tsc/validate = 0` + the config seam being genuinely parameterized (proven: the
presets take server command / base URL / test dir / docker template / warmup as
config, zero hardcoded ziee paths).

## Design-gate — no Rust/OpenAPI/generated-`types.ts` impact

Pure test tooling. `git status` shows ZERO changes to any `api-client/types.ts`,
`openapi.json`, `.rs`, migration, or `.sql` — and the moved generators reproduce
their gallery artifacts byte-identically, so none of `testIds.generated.ts` /
`galleryCoverage.generated.ts` / `stateMatrix.generated.ts` /
`overlay-registry.generated.json` changed on disk either.

SDK commit: `0174b83ab674725dd2b6844a47111c71c64a00cc` (branch `sdk-testinfra`, local, not pushed).
