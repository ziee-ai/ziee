# Chunk `sdk-gallery-b3` — desktop gallery rewire + testid-generator move (CUT manifest)

Closes the two `@ziee/gallery` deferrals (B-3 from the `sdk-gallery` BOUNDARY +
`gen-testid-registry` from the `sdk-testinfra` BOUNDARY). Both additive /
backward-compat; pure frontend (ZERO Rust/OpenAPI/generated-`types.ts` impact).

## Deliverable 1 — B-3: full desktop gallery rewire onto `@ziee/gallery` (MOVE)

The web gallery already boots via `mountGallery(buildGalleryConfig())`; the
DESKTOP gallery still ran its OWN local framework copies (kept green by the web
`support/registry-core.ts` re-export shim). This rewires desktop onto the package
with the desktop's own `discoverGalleries` (the cross-workspace cassette merge)
and retires the shim. `@ziee/gallery` stays workspace-agnostic; desktop supplies
its merging config.

| ziee source (DELETED — framework copy, now owned by `@ziee/gallery`) | replaced by |
|---|---|
| `desktop/ui/src/dev/gallery/GalleryPage.tsx` | `@ziee/gallery` `GalleryPage` (via `mountGallery`) — no in-app route ⇒ no shim |
| `desktop/ui/src/dev/gallery/pages.tsx` | `@ziee/gallery` `runtime/pages` |
| `desktop/ui/src/dev/gallery/overlays.tsx` | `@ziee/gallery` `registry/types` (empty desktop overlay class) |
| `desktop/ui/src/dev/gallery/surfaces.ts` | `@ziee/gallery` `runtime/surfaces` |
| `desktop/ui/src/dev/gallery/matrix.ts` | `@ziee/gallery` `runtime/matrix` |
| `desktop/ui/src/dev/gallery/useGalleryTheme.ts` | `@ziee/gallery` `runtime/useGalleryTheme` |
| `desktop/ui/src/dev/gallery/story.tsx` | `@ziee/gallery` `runtime/story` (no desktop stories ⇒ full delete, not shim) |
| `desktop/ui/src/dev/gallery/seed.ts` | folded into `galleryConfig.ts` (`seedAuth`) + `mountGallery` |
| `ui/src/dev/gallery/support/registry-core.ts` (web re-export SHIM) | RETIRED — its sole consumer (desktop `module-seed.ts`) now imports `@ziee/gallery` directly |

MODIFIED / NEW (ziee-side binding — the equivalence mechanism):

- `desktop/ui/src/dev/gallery/main.tsx` — rewritten to the thin
  `mountGallery(buildGalleryConfig())` boot (mirrors the web `main.tsx`).
- `desktop/ui/src/dev/gallery/galleryConfig.ts` (NEW, 123 L) — the desktop's
  `GalleryConfig` DI: `discoverGalleries` + core+desktop module loaders + router
  store + auth seed + ThemeProvider/ErrorBoundary/Loading/LazyRenderer + accents
  + theme writers + `crawlCassette`. Page-focused (`stories`/`deepState` omitted).
- `desktop/ui/src/dev/gallery/module-seed.ts` — now exports `discoverGalleries()`
  (page-focused cross-workspace merge); imports the pure merge/assert straight
  from `@ziee/gallery` (not the retired web shim).
- `desktop/ui/src/dev/gallery/mockApi.ts` — collapsed from the full 342-L engine
  to a ~40-L binding SHIM (binds `Cassette` to the desktop api-client, re-exports
  the `@ziee/gallery` engine) — mirrors the web `mockApi.ts` shim.
- `desktop/ui/src/dev/gallery/fixtures/index.ts` — dropped the now-unused
  `GALLERY_CASSETTE` (mount composes it from `crawlCassette` + `discoverGalleries`);
  keeps the auth-seed re-export.

## Deliverable 2 — gen-testid-registry: resolve the kit-migration blocker + MOVE

The `sdk-testinfra` chunk deferred this: ziee's `ui/scripts/gen-testid-registry.mjs`
wrote the DELETED path `src/components/ui/testIds.generated.ts` and walked only
ui+desktop (yielding a set OUT of sync with the committed
`sdk/packages/kit/src/testIds.generated.ts`, 1590 ids). **Reconciled — no genuine
conflict:** the committed kit registry is EXACTLY `union(ui/src, desktop/ui/src,
kit/src, shell/src)` under the same walk exclusions (proven byte-identical, set +
order). The two-id gap was the kit/shell PACKAGE-src testids (`app-root`,
`app-sidebar`, `layout-sidebar-resize-handle` in `@ziee/shell`; the pagination
`${testid}-row-…` literal in `@ziee/kit`) the app-only walk can't see.

| ziee source (DELETED) | SDK dest | anchor change |
|---|---|---|
| `ui/scripts/gen-testid-registry.mjs` | `packages/gallery/scripts/gen-testid-registry.mjs` | `HERE`-relative `UI_SRC`/`DESKTOP_SRC`/`OUT` → `resolveGalleryConfig(cwd)`: app trees = `srcDir`+`extraTrees`, package trees = **`kitTestIds`** (NEW), output = **`testidOut`** (NEW) |
| — (none) | `packages/gallery/scripts/gen-testid-registry.test.mjs` (NEW) | pure-fn unit test (`collectSourceFiles`/`collectTestIds`/`renderRegistry`) |

New config fields on `scripts/lib/gallery-config.mjs` (both default to the
historical in-app behavior → a config-less app is unchanged):
**`kitTestIds: []`** (package src roots to also walk), **`testidOut: null`**
(→ `<srcDir>/components/ui/testIds.generated.ts` default; ziee sets the kit path).

ziee repoint (equivalence surface):
- `ui/package.json` — `check/gen:testid-registry` → `../../sdk/packages/gallery/scripts/gen-testid-registry.mjs`.
- `desktop/ui/package.json` — `check:testid-registry` → the package path (+ added `gen:testid-registry` for parity).
- `ui/gallery.config.json` — added `kitTestIds` + `testidOut` (paths relative to `ui/`).
- `desktop/ui/gallery.config.json` (NEW) — the mirrored config (paths relative to `desktop/ui/`); the sorted UNION is identical from either cwd, so the single committed registry is byte-stable regardless of which workspace's `--check` runs.

## Design-gate — the testid kit-migration was RECONCILED, not force-fit (TOP RISK)

The `sdk-testinfra` STOP-rule said: STOP if the kit-migration state can't be
cleanly reconciled (a genuine id conflict, not just a path). It was PROVEN clean:
`union(ui/src, desktop/ui/src, kit/src, shell/src)` == the committed kit registry,
**byte-identical** (1590 ids, same order). The moved generator run in write mode
from BOTH `ui/` and `desktop/ui/` cwds reproduces the committed
`sdk/packages/kit/src/testIds.generated.ts` with a ZERO diff — so nothing about the
kit file changed; only the generator's home + its walked roots did. `--check`
passes from both cwds. No wrong registry was forced.

## Design-gate — desktop renders THROUGH `@ziee/gallery`, not the shim (xvfb)

Booted the desktop gallery Vite dev server + drove it headless under
`xvfb-run`. Evidence the render goes through the PACKAGE `GalleryPage`, not the
deleted local copy:
- the rendered `[data-testid="gallery-root"]` carries
  `data-gallery-build-marker="ZIEE_GALLERY_SEED_MARKER"` — a marker the PACKAGE
  `GalleryPage` emits and the old desktop `GalleryPage.tsx` NEVER had;
- 50 `gallery-page-*` surfaces enumerate;
- the 14 desktop gallery e2e specs pass (2 seed + 6 override + 6 runtime
  render/axe), zero console/page error, axe clean, light+dark — proving the
  desktop-only OVERRIDE surfaces + the cross-workspace shared cassette
  (`settings-js-tool`) render through the package.

## Design-gate — page-focus preserved (web overlay entries stay excluded)

Desktop's `discoverGalleries` returns ONE synthetic entry carrying the
cross-workspace merged CASSETTE (shared web + desktop, desktop-wins) + the
desktop-only galleries with their cassette OMITTED. So shared web PAGES render
populated but the web overlay/deep/seeded/story ENTRIES are NOT inherited (desktop
is a pages-only canvas) — identical to the pre-rewire `module-seed.ts` behavior.
The synthetic-single-entry shape also sidesteps `mergeModuleCassettes`' collision
THROW (which the old `{...shared, ...desktop}` spread silently tolerated).

## Design-gate — no Rust/OpenAPI/generated-`types.ts` impact

Pure frontend. `git status` shows ZERO changes to any `api-client/types.ts` /
`openapi.json` / `.rs` / migration / `.sql`, and the kit `testIds.generated.ts`
is byte-identical to its committed form → no generated file changed on disk.
`vendor/pgvector` NOT touched/staged.

SDK commit: local on branch `sdk-gallery-b3` (not pushed).
