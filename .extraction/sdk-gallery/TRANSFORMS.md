# Chunk `sdk-gallery` — TRANSFORMS

Every symbol whose SDK form differs from its pre-move ziee form, and why. The
render frames, mock state-modes, SSE replay, safe-empty proxy, interaction
engine, story layout, and the runtime-health in-page audit are byte-for-byte
preserved; the transforms are the mechanical de-coupling (generic type param,
`@/` → DI, glob → injection, hardcode → config) the package boundary requires.

- **T-1 `Cassette` → `Cassette<TResponses>`** (`mock/mockApi.ts`). Was
  `{ [K in ApiEndpoint]?: GetResponseType<K> | resolver }` over the app's
  generated `ApiEndpoint`/`GetResponseType`. Now generic over the app's response
  map with a `Record<string,unknown>` default; runtime fns take `Cassette<any>`
  (type-erased). **why:** the package can't import `@/api-client/types`. The
  compile-time check moves to the authoring site via the app's binding alias
  (`support/types.ts`: `type Cassette = GCassette<ApiEndpointResponses>`).
  **Decision D-1 (resolved):** route the seam through a package generic rather
  than `@ziee/framework/api-client`'s `createApiClient<T>` — the cassette needs
  the RESPONSE map, not the client factory; a generic `<TResponses>` is the
  minimal, exact carrier. Proven: wrong-shape scratch fails `tsc`.

- **T-2 `ModuleGallery` → `ModuleGallery<TResponses>`** (`registry/types.ts`).
  `cassette?: Cassette<TResponses>`; other fields unchanged. App binds
  `type ModuleGallery = GModuleGallery<ApiEndpointResponses>`. **why:** carries
  T-1's generic to the per-module authoring contract; the 36 `gallery.tsx` files
  are byte-unchanged (they import the bound alias from `@/dev/gallery/support`).

- **T-3 mock engine: module-const route table → `configureMockApi(...)`**
  (`mock/mockApi.ts`). The `COMPILED` route array (was built at import from the
  static `ApiEndpoints`) is now built by `configureMockApi({ apiEndpoints,
  specialRoutes, errorModeExempt })`, called once by `mountGallery`. **why:** the
  route table is app-generated; it must be injected, not imported.

- **T-4 inline SSE/PDF/text branches → `SpecialRoute[]` DI** (`mock/mockApi.ts`).
  The hardcoded `/api/chat/stream`, `/api/files/{id}/raw` (SAMPLE_PDF), and
  `/api/files/{id}/text` blocks (which imported `@/modules/file/.../pdf-fixture`)
  become injected `specialRoutes`, iterated first-match BEFORE the state-mode
  logic — the exact original order. ziee registers the same four handlers in
  `galleryConfig.ts` using the package's exported `sseReplayResponse` /
  `makeBinaryResponse` / `jsonResponse` / `mockErrorResponse`. **why:** those
  endpoints + the PDF/canvas fixtures are ziee content. **Decision D-2 (resolved):**
  keep the generic SSE-replay + binary helpers IN the package (they are
  mechanism, not content); only the app-specific PATHS + fixture bytes are DI'd.

- **T-5 `main.tsx` + `seed.ts` → `mountGallery(cfg)`** (`runtime/mount.tsx`).
  The URL-param parse, auth precedence, `setMockMode`, seed (installMockApi +
  seedAuth + loadModules), surface-globals publish, and `ReactDOM.render` are
  folded into one boot fn taking `GalleryConfig`. The idempotent-`seeded` guard,
  auth precedence (`?auth=` → `surfaceAuthSeed` → admin), and render tree
  (`StrictMode > ErrorBoundary > ThemeProvider > GalleryPage`) are preserved.
  `@/index.css` import stays in the app's thin `main.tsx`. **why:** the boot is
  generic; the DI is the app's.

- **T-6 page/overlay/deep/seeded frames → DI'd, unified in `pages.tsx`**
  (`runtime/pages.tsx`). `AppErrorBoundary`/`Loading`/`LazyComponentRenderer`/
  `useRoutesStore` → `getGalleryConfig()` fields; `PARAM_VALUES.providerId` →
  `cfg.paramValues`; `SKIP_PATHS` → `cfg.skipPaths`. The deep frame's hardcoded
  `ConversationPage` + `/chat/:conversationId` → `cfg.deepState.{component,
  routePath,buildInitialPath}`. Frame JSX/testids/heights byte-identical.
  **why:** the frames are generic; the components + params are the app's.

- **T-7 `AccentPreset` → `string`; `parseGalleryParams` gains accent params**
  (`runtime/matrix.ts`). The accent list/labels/default (were `ACCENT_ORDER`/
  `ACCENT_PRESETS` from `@/components/ThemeProvider`) become `cfg.accents` /
  `accentLabels` / `defaultAccent`; `parseGalleryParams(search, accents,
  defaultAccent)`. Theme/viewport/dir constants unchanged. **why:** accents are
  app tokens. `themeToPreference` (an identity fn over `ThemePreference`) dropped
  — `useGalleryTheme` calls `cfg.setThemePref('light'|'dark')` directly.

- **T-8 `support/registry.ts` singletons → assembled at mount**
  (`runtime/surfaces-registry.ts`). `OVERLAY_ENTRIES`/`DEEP_STATE_ENTRIES`/
  `SEEDED_SURFACE_ENTRIES`/`MODULE_STORIES`/`MODULE_CASSETTE` (were module-const
  from the eager glob) → `initSurfaces(discovered)` + getters, called by
  `mountGallery`. The app-side `support/registry.ts` keeps the glob + still
  exports `MODULE_CASSETTE` (via the package's `mergeModuleCassettes`, safe-cast
  to the app's `Cassette`) + `MODULE_GALLERIES` for the desktop cross-workspace
  bridge, + NEW `discoverGalleries` for `mountGallery`. **why:** the glob is
  app-side; the assembly is generic.

- **T-9 `GalleryPage.tsx` in-app entry → config-priming shim** (app-side).
  The in-app `/dev/gallery` route renders `GalleryPage` WITHOUT the standalone
  boot, so the app-side `GalleryPage.tsx` shim calls `setGalleryConfig(cfg)` +
  `initSurfaces(cfg.discoverGalleries())` at module-eval (no mock install — the
  in-app route renders against the live app) then re-exports the package
  `GalleryPage`. **why:** `getGalleryConfig()` must be primed for both the
  standalone (`mountGallery`) and the in-app entry.

- **T-10 gate scripts: hardcodes → `gallery.config.json`** (`scripts/{gate-ui,
  runtime-health}.mjs` + `lib/gallery-config.mjs`). The `GALLERY_DIR`/`PORT`/
  `OUT`/gallery-URL anchors, the static `runtime-baseline.js` import, the dev
  boot command, the visual config + spec list, and the lint commands become
  `resolveGalleryConfig()` fields (defaulting to ziee's exact historical values).
  The runtime-health in-page audit + harness-noise classifier + gate PASS/FAIL
  table are byte-preserved. **why:** paths/ports/baseline are per-app.

## Decision Resolution (zero TBD)

- **D-1 Cassette seam** — RESOLVED: generic `Cassette<TResponses>` + app binding
  alias. Proven `tsc`-fails on wrong shape. (Not `createApiClient<T>` — see T-1.)
- **D-2 special routes** — RESOLVED: SSE/binary helpers stay in-package;
  app-specific paths + fixtures DI'd via `specialRoutes` (T-4).
- **D-3 desktop** — RESOLVED (minimal, identical-behavior): desktop's local
  framework copies retained + compile via a `support/registry-core.ts` re-export
  shim; desktop's `module-seed.ts` cross-workspace cassette bridge unchanged. The
  FULL desktop rewire to `mountGallery` is a declared follow-up (BOUNDARY B-3) —
  desktop stays byte-behavior-identical meanwhile.
- **D-4 prod marker** — RESOLVED: fixed literal in the package; `prodMarker`
  config defaults to it (must match). Proven emitted at runtime.
