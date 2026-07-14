# Chunk `sdk-gallery-b3` — TRANSFORMS

Every symbol whose realized form differs from its pre-chunk ziee form, and why.
The framework BODIES are unchanged (they already live, byte-for-byte, in
`@ziee/gallery` from the `sdk-gallery` chunk); the desktop transforms are the
mechanical binding of the desktop app to the package's `mountGallery(cfg)` seam.
The testid transform is the config-anchor de-coupling (`HERE`-hardcode →
`resolveGalleryConfig(cwd)` + config fields) the package boundary requires, plus
the kit-migration reconciliation (the walk now unions the kit/shell package src).

## Deliverable 1 — desktop rewire

- **T-1 desktop boot** (`desktop/ui/src/dev/gallery/main.tsx`). Was a ~86-L file
  that hand-assembled the surface manifest globals + `seedGallery` + `setMockMode`
  + `ReactDOM.createRoot(...).render(<GalleryPage/>)`. Now the thin
  `mountGallery(buildGalleryConfig())` (mirrors the web `main.tsx`). **why:** the
  package's `mountGallery` folds the former `main.tsx` + `seed.ts` (surface
  assembly → configure/install mock → seed auth → load modules → render under the
  app ThemeProvider). Identical ordering.

- **T-2 `buildGalleryConfig()`** (`desktop/ui/src/dev/gallery/galleryConfig.ts`,
  NEW). The desktop `GalleryConfig` DI, folding the deleted `seed.ts`'s `seedAuth`
  (admin/limited/none) + the loaders. `loadModules` = `loadModules()` +
  `loadDesktopModules()` (the desktop bootstrap: core + desktop-only modules);
  `crawlCassette` injected as `GalleryConfig.crawlCassette`; ThemeProvider /
  AppErrorBoundary / Loading / LazyComponentRenderer / useRoutesStore / accents /
  theme writers resolved through the desktop `@/` space (override→web where a
  desktop copy is absent). **why:** everything the framework used to reach through
  `@/` is now injected. **Decision D-1 (resolved):** desktop is PAGE-FOCUSED, so
  `stories` + `deepState` are OMITTED (no desktop kit-stories, no desktop
  active-conversation deep-states) — the package tolerates their absence
  (`stories ?? []`, and `deepState` is only read when a deep entry is requested).
  `paramValues` is also omitted (desktop `pages.tsx` only ever used URL params;
  the package merges `paramValues` over `urlParams()`, so omitting = URL-only,
  unchanged).

- **T-3 `discoverGalleries()`** (`desktop/ui/src/dev/gallery/module-seed.ts`).
  Was a module exporting only `MODULE_CASSETTE = {...SHARED_CASSETTE,
  ...mergeModuleCassettes(DESKTOP_GALLERIES)}`, imported into
  `fixtures/index.ts`'s `GALLERY_CASSETTE`. Now ALSO exports `discoverGalleries()`
  → `[{ module:'__desktop_merged_cassette__', gallery:{ cassette: MODULE_CASSETTE } },
  ...DESKTOP_GALLERIES.map(strip-cassette)]`. **why:** the package's
  `initSurfaces(discoverGalleries())` re-runs `mergeModuleCassettes` over the
  returned list. Passing the pre-merged cassette as ONE synthetic entry (a)
  preserves the "desktop wins on key overlap" semantics the old spread had —
  feeding shared + desktop as SEPARATE entries would trip `mergeModuleCassettes`'
  duplicate-key THROW — and (b) keeps it PAGE-FOCUSED (the synthetic entry carries
  no overlay/deep/seeded/story class, and the desktop-only entries have none
  today). **Decision D-2 (resolved):** the merge/assert now import DIRECTLY from
  `@ziee/gallery` (was `@/dev/gallery/support/registry-core`, the retired web
  shim). `MODULE_CASSETTE` stays exported (still the source of the merged cassette)
  though `fixtures/index.ts` no longer consumes it — it is the internal input to
  `discoverGalleries`; NOT dead (re-exported for parity with the web registry +
  used within the module).

- **T-4 `mockApi.ts` shim** (`desktop/ui/src/dev/gallery/mockApi.ts`). The 342-L
  engine (route matching, state modes, SSE replay, safe-empty proxy) collapsed to
  a ~40-L binding shim: `Cassette`/`CassetteEntry` bound to the DESKTOP
  `ApiEndpointResponses`, everything else re-exported from `@ziee/gallery`.
  **why:** the engine is the package's; only the app-type binding stays app-side
  (the compile-time cassette-shape check). Byte-mirrors the web `mockApi.ts` shim.
  The desktop fixtures (`auth.ts`/`citations.ts`/`crawl.generated.ts`) + the
  `gen-crawl-cassette.mjs` emitter still `import type { Cassette } from '../mockApi'`
  unchanged.

- **T-5 `fixtures/index.ts`** — dropped `GALLERY_CASSETTE` (+ its `MODULE_CASSETTE`
  and `crawlCassette` imports). **why:** `mountGallery` now composes the cassette
  from `GalleryConfig.crawlCassette` (the injected crawl base) + the module
  cassette (via `discoverGalleries`), so a pre-assembled `GALLERY_CASSETTE` is dead.
  Keeps the auth re-export (`adminUser`/`adminMe`/`adminPermissions`).

- **T-6 shim retirement** (`ui/src/dev/gallery/support/registry-core.ts` DELETED).
  It existed ONLY so desktop's `module-seed.ts` could reach the pure merge/assert
  via the `@/` override fallback (`sdk-gallery` D-3/FIX-1.3). Desktop now imports
  those from `@ziee/gallery` directly, so the shim has zero consumers. **why:** the
  re-export duplicate is removed; the package is the single home. Verified no other
  consumer (the surviving `registry-core` references are all inside the package's
  own `registry/registry-core.ts`).

## Deliverable 2 — gen-testid-registry (config-anchor + kit-migration reconcile)

- **T-7 path anchors** (`gen-testid-registry.mjs`, moved). Was
  `HERE=dirname(import.meta.url)`, `UI_SRC=HERE/../src`,
  `DESKTOP_SRC=HERE/../../desktop/ui/src`, `OUT=UI_SRC/components/ui/testIds.generated.ts`
  — all `HERE`-relative (cwd-independent, which is why it produced the same output
  from either workspace). Now `resolveGalleryConfig(cwd)`-driven: app trees =
  `[srcDir, ...extraTrees].map(resolve-against-cwd)`, package trees =
  `kitTestIds.map(...)`, output = `resolve(cwd, testidOut ?? <srcDir>/components/ui/…)`.
  **why:** moved to the package, `HERE` is the package dir; the app's cwd +
  `gallery.config.json` must drive paths. The `HERE`-cwd-independence is preserved
  a different way: BOTH `ui/gallery.config.json` and `desktop/ui/gallery.config.json`
  describe the SAME union (each with cwd-relative roots), so the sorted output is
  byte-identical from either cwd. **Decision D-3 (resolved):** the walk now UNIONS
  the kit/shell PACKAGE src (`kitTestIds`) with the app trees — this is the
  kit-migration reconciliation: the kit-component testids moved into `@ziee/kit` /
  `@ziee/shell`, so their own literals are the source of truth for kit-component
  ids; the app walk adds the app's ids; the union == the committed kit registry
  (byte-proven). The `sdk-testinfra` D-4 deferral is thereby CLOSED.

- **T-8 pure-fn extraction + render** (`gen-testid-registry.mjs`). The inline
  `walk`/`LITERAL`/body-render logic is refactored into exported pure functions
  `collectSourceFiles(dir)` / `collectTestIds(files)` / `renderRegistry(sorted)`
  (for the unit test), and a portable `isMain` guard
  (`import.meta.url === pathToFileURL(process.argv[1]).href`) mirroring
  `gen-gallery-seed-registry.mjs` — the naive `file://${argv[1]}` main check is
  false on Windows/space paths. The regex, exclusions (node_modules/dist/build/
  .git/tests, `src/dev`, `gallery.{ts,tsx}`), and the emitted body FORMAT are
  byte-preserved (+ an explicit `testIds.generated.ts` basename skip so walking a
  package's own output never feeds it back — a no-op for the current set since
  array entries aren't `data-testid=` literals, but intent-clarifying). The
  rendered header string ("… across the ui + desktop trees") is kept VERBATIM so
  the committed kit file is byte-unchanged. **Decision D-4 (resolved):** kept the
  header verbatim rather than "correcting" it to mention kit/shell — accuracy is
  documented in the source comments; changing the emitted string would rewrite the
  committed kit registry for zero functional gain and risk a spurious `--check`
  drift.

- **T-9 config fields** (`scripts/lib/gallery-config.mjs`). Added `kitTestIds: []`
  + `testidOut: null` to `DEFAULTS`. **why:** the moved generator needs the package
  roots + the output path; both defaults reproduce the pre-package in-app behavior
  (empty package walk + `<srcDir>/components/ui/testIds.generated.ts`) → a
  config-less app is unchanged. This is the sibling of the `sdk-testinfra` D-3
  (`testidOut` was ADDED-then-removed there when the move was deferred; it is now
  RE-ADDED, its purpose realized).
