# Chunk `sdk-testinfra` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form, and why. The
generator BODIES (walk logic, AST extraction, regex classification, render
templates) are byte-for-byte preserved; the transforms are the mechanical
de-coupling (`HERE`-hardcode → `resolveGalleryConfig(cwd)` + config field) the
package boundary requires. The test-e2e transforms are the parameterization of
ziee's e2e scaffold hardcodes into the `E2EConfig` seam.

## B1 — moved generators (`HERE`-anchor → config-anchor)

- **T-1 `gen-state-matrix` path anchors** (`gen-state-matrix.mjs`). Was
  `HERE=dirname(import.meta.url)`, `UI_DIR=HERE/..`, `SRC=UI_DIR/src`, hardcoded
  `dev/gallery/*` outputs + literal `GLOBS=['modules/**/*.tsx','components/ui/**/*.tsx']`
  + `tsConfigFilePath=UI_DIR/tsconfig.json`. Now `UI_DIR=CFG.__cwd`,
  `SRC=resolve(cwd,srcDir)`, outputs under `resolve(cwd,galleryDir)`, `GLOBS`
  derived from `surfaceRoots`, tsconfig from `CFG.tsconfig`. **why:** moved to the
  package, `HERE` is the package dir; the app's `ui/` cwd must drive paths. AST
  extraction unchanged. **Decision D-1 (resolved):** derive `GLOBS` as
  `surfaceRoots.map(r => join(resolve(cwd,r),'**/*.tsx'))` (absolute) rather than
  keeping `join(SRC,glob)` — a single source (`surfaceRoots`) for the walked roots,
  identical resolved set. Proven byte-identical.

- **T-2 `gen-overlay-registry` roots + kit-import predicate** (`gen-overlay-registry.mjs`).
  `ROOTS` derived from `surfaceRoots` (first root skips `module.tsx`); the
  inline `source === '@/components/ui' || startsWith('@/components/ui/') ||
  source === '@/modules/layouts/app-layout/components/Drawer'` became
  `isKitSource(source)` over `CFG.overlayKitImports` (each `P` matches
  `source===P || startsWith(P+'/')`). `MODULES_DIR` from `surfaceRoots[0]`. **why:**
  the overlay-import specifiers are app-specific; `overlayKitImports` defaults to
  ziee's exact list → byte-identical. Exported `extractWiredSurfaces` unchanged.
  **Decision D-2 (resolved):** dropped the unused `KIT_IMPORT_RE` const (defined
  but never referenced in the original); no behavioral effect.

- **T-3 `gen-gallery-coverage` roots + outputs** (`gen-gallery-coverage.mjs`).
  `SRC`/`OUT`/`COVERAGE` from `srcDir`+`galleryDir`; `ROOTS` from `surfaceRoots`
  (first skips `module.tsx`). Walk + union render unchanged. Proven byte-identical.

- **T-4 `gen-gallery-seed-registry` src default** (`gen-gallery-seed-registry.mjs`).
  Default `SRC=HERE/../src` → `resolve(CFG.__cwd, CFG.srcDir)`; the `--src <dir>`
  override now resolves against `CFG.__cwd` (== `process.cwd()`, unchanged
  semantics). `MODULES_DIR`/`OUT`/`EXCEPTIONS` still anchored on `SRC` (NOT
  `galleryDir`), preserving the desktop `--src src` behavior exactly. Pure exports
  (`hasUserSurface`/`hasSeed`/`parseSeedExceptions`/`computeSeedDrift`/
  `enumerateModules`) unchanged. **why:** `--src` is how the desktop workspace runs
  ONE gate over its own tree; keeping outputs relative to `SRC` (not a separate
  `galleryDir`) is what makes `desktop/ui/ --src src` land in `desktop/ui/src/dev/gallery/`.
  `--check` passes from both cwds.

- **T-5 `gallery-coverage` anchors** (`gallery-coverage.mjs`). `SRC`/`GALLERY`
  from `srcDir`+`galleryDir`. `isSurfaceFile`'s `/\/src\/modules\//` +
  `/\/src\/components\/ui\//` regexes kept literal (report-only tool, not a
  committed-artifact gate) — informational branch-coverage output, byte-identical
  for ziee. Imports the package `lib/gallery-surfaces.mjs` (already present).

- **T-6 `check-gallery-prod-exclusion` anchors** (`check-gallery-prod-exclusion.mjs`).
  `DIST=resolve(cwd,distDir)`, `MARKER=CFG.prodMarker`, build via `CFG.buildCmd`.
  Walk + leak-detection unchanged. **why:** the dist dir, sentinel string, and
  build command are all app-specific; defaults = ziee's exact values.

- **T-7 captures verbatim** (`capture-gallery-{screenshots,states}.mjs`). No
  transform — they already take `--url`/`--out` CLI args and import
  `./lib/gallery-surfaces.mjs` (present in the package). Copied byte-identical.

- **T-8 `gallery-config.mjs` new fields** (`scripts/lib/gallery-config.mjs`).
  Added `srcDir:'src'`, `overlayKitImports:['@/components/ui','@/modules/layouts/app-layout/components/Drawer']`,
  `tsconfig:'tsconfig.json'` to `DEFAULTS`. **why:** the moved generators need
  these anchors; every default = ziee's historical hardcode → a config-less app
  is unchanged. **Decision D-3 (resolved):** a `testidOut` field was added then
  REMOVED when `gen-testid-registry` was deferred (T-9) — no dead config surface.

- **T-9 `gen-testid-registry` NOT moved** (deferred). Attempted move produced
  1588 ids vs the committed kit file's 1590 (the kit file includes kit-package-src
  ids the ui+desktop walk can't see). **Decision D-4 (resolved):** leave it
  app-side untouched — it's mid kit-extraction migration; cleanly parameterizing
  it means resolving that migration (out of scope). ziee's testid check behaves
  identically (unchanged file). Recorded in BOUNDARY.

## B2 — playwright templates (NEW, config-driven)

- **T-10 `defineVisualConfig`** (`playwright/visual.config.ts`). The config-driven
  form of ziee's `playwright.visual.config.ts`: reads `gallery.config.json`
  (`port`/`galleryUrl`/`visualTestDir`/`devCmd`/`maxDiffPixelRatio`) with defaults
  matching ziee's literals; `webServer.command` built from `devCmd`. **why:** a
  fresh app's visual config becomes `export default defineVisualConfig()`. ziee's
  own visual config is NOT repointed (kept app-side; the gate reads it as
  `CFG.visualConfig`) — repoint is a trivial future flip. `tsc = 0` (temp project).

- **T-11 generic spec templates** (`playwright/templates/*.template.ts`).
  Surface-list-driven (`window.__GALLERY_LIST_ALL_SURFACES__()`) Layer-A
  (layout+axe) / states / overlays / Layer-B starters. **why:** ziee's real specs
  are baseline-coupled (`layout-baseline`/`axe-baseline`/`_gallery.ts` accent
  matrix) and STAY app-side; the templates are the generic skeleton a new app
  copies (drop `.template`, add baselines). Not package-compiled (templates import
  `./_gallery`, resolved post-copy).

## Deliverable 1 — test-e2e parameterization (`E2EConfig` seam)

- **T-12 `E2EConfig`** (`test-e2e/src/config.ts`). The union of ziee's e2e
  hardcodes as ONE config: `testDir`/`testIgnore`/`baseURL`/`timeout`/`workers`/
  `viewport`/`webServer` (preset) + `dockerComposeTemplate`/`containerNamePrefix`/
  `basePgPortEnv`/`configDir`/`envFile`/`uiBuild`/`serverWarmup`/`firstRunSetup`
  (global-setup). **why:** the seam. No ziee path is baked in.

- **T-13 `createGlobalSetup(cfg)` / `createGlobalTeardown(cfg)`**
  (`test-e2e/src/global-{setup,teardown}.ts`). Factory form of ziee's
  `global-setup.ts`/`global-teardown.ts`: docker container reap keyed by
  `containerNamePrefix`+session-ns, PG bring-up from `cfg.dockerComposeTemplate`,
  UI build from `cfg.uiBuild` (dedupe list injected, not the antd-specific literal),
  server warmup from `cfg.serverWarmup.command`, first-run via `cfg.firstRunSetup`.
  `pg`/`dotenv` imported LAZILY so a no-DB suite needn't install them. **Decision
  D-5 (resolved):** ship a LEAN generic `port-manager.ts` (the shared-lock
  `{pid,runId}` liveness core) rather than porting ziee's 563-line fixture — the
  scaffold needs only PG-port allocate/release + stale-lock/config cleanup; an app
  with per-worker vite+backend port pairs keeps its own richer fixture.

- **T-14 `definePlaywrightPreset` / `defineDesktopPreset`**
  (`test-e2e/src/playwright-preset.ts`). Parameterized `defineConfig` from ziee's
  `playwright.config.ts` (full-stack: workers=1 default, 180s, per-run output dirs,
  `PLAYWRIGHT_WORKERS` env override) + `desktop/ui/playwright.config.ts` (desktop:
  `webServer` Tauri/headless boot, `baseURL`, 300s). `globalSetup`/`globalTeardown`
  passed as MODULE PATHS via `PresetPaths` (Playwright requires file paths there,
  not fn refs). **why:** both OS variants, app-agnostic.

- **T-15 selectors** (`test-e2e/src/testid.ts`). `byTestId` generic over
  `TestIdLike<Known=string>` + `makeByTestId<Known>()` an app binds to its
  generated union (compile-time typo-check preserved app-side) + `byRole`/`byLabel`/
  `byText` (the accessibility-first ladder). **why:** the app owns its typed union
  (`@ziee/kit/testIds.generated`); the package owns the helpers.
