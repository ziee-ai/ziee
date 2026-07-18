# Chunk `sdk-gallery` — DRIFT round 1

Drift = any behavioural divergence between the pre-move gallery framework and the
extracted package + app shims, checked symbol-by-symbol against the pre-move
files and confirmed by the live smoke + the config-driven runtime-health run.

- **DRIFT-1.1** — verdict: none. `mockApi` engine: route compilation, the
  most-specific `matchRoute` tie-break, state modes (loaded/empty/error/delayed),
  `toEmpty`, `makeSafeEmpty` proxy, and the SSE-replay wire format are byte-copied.
  The only structural change is the route table + SSE/PDF/text branches becoming
  injected (`configureMockApi` + `specialRoutes`); ziee registers the identical
  four handlers with the identical fixtures, so request→response is unchanged.
- **DRIFT-1.2** — verdict: none. The four surface frames (page/overlay/deep/
  seeded): JSX, testids (`gallery-page-<slug>`), `data-gallery-state`, heights
  (720 / fullHeight), settle timings, crash-fallback marker — identical. The deep
  frame's mount target is now `cfg.deepState` (ziee → ConversationPage +
  `/chat/:conversationId`) = the pre-move hardcode.
- **DRIFT-1.3** — verdict: none. `useResolvedPages` slug/skip/param logic
  (`slugFor` `-detail` suffix, `resolveInitialPath`, sort) unchanged; the smoke
  enumerated 43 pages (the same count) with the same slugs (`auth`/`chat`/`chats`
  /`hub`/`knowledge` sampled).
- **DRIFT-1.4** — verdict: none. Surface-globals: `__GALLERY_OVERLAYS__` /
  `_DEEP_STATES__` / `_SEEDED__` / `_INTERACTIONS__` / `_LIST_ALL_SURFACES__`
  published by `mountGallery` (was `main.tsx`); the smoke confirmed all present
  with 47/17/99 counts + the function form of `listAllSurfaces`.
- **DRIFT-1.5** — verdict: none. Theme matrix: `GALLERY_THEMES`/`_VIEWPORTS`/
  `_DIRS` + `parseGalleryParams` validation identical; the accent set/labels are
  now injected but equal `ACCENT_ORDER`/`ACCENT_PRESETS` (ziee passes them
  verbatim in galleryConfig). Default accent `blue` preserved.
- **DRIFT-1.6** — verdict: intended, inert. The in-app `/dev/gallery` route now
  primes the framework config at the `GalleryPage.tsx` shim's module-eval
  (`setGalleryConfig` + `initSurfaces`, NO mock install). Pre-move, the in-app
  GalleryPage read ziee modules directly and rendered against the live backend;
  the shim reproduces exactly that (config for accents/stories + surfaces for
  bySlug lookups) without installing the mock — so the in-app route's live-backend
  behavior is preserved.
- **DRIFT-1.7** — verdict: none. Story/interaction/lazy/hold: `story.tsx` +
  `interactions.ts` + `lazy.tsx` + `hold.ts` moved verbatim (interactions.ts is a
  literal `cp`); testids `gallery-section-<id>` / `gallery-case-<id>-<key>`
  unchanged; the smoke rendered 72 story sections.
- **DRIFT-1.8** — verdict: none. Gate scripts: the runtime-health in-page audit
  (contrast/a11y-name/off-grid), harness-noise classifier, baselined subtraction,
  per-surface verdict, and gate-ui PASS/FAIL table are byte-preserved; only the
  path/port/baseline/dev-cmd/visual anchors became config lookups defaulting to
  ziee's values. The config-driven runtime-health reproduced a clean hub verdict
  against the live gallery.
- **DRIFT-1.9** — verdict: none. `MODULE_CASSETTE` (app support/registry) is now
  `mergeModuleCassettes(DISCOVERED) as Cassette` (the merge fn moved to the
  package; the cast restores the app-strict type for the fixtures barrel + the
  desktop bridge). Same runtime value (a plain endpoint→response map).

**Unresolved drifts:** 0
