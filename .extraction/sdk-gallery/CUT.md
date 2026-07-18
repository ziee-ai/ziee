# Chunk `sdk-gallery` — the gallery / visual-testing FRAMEWORK (CUT manifest)

Cut the **generic render-every-surface × states × themes** framework (the runner,
the mock-API engine, the pure registry, the four surface-frame types, the theme
matrix, the interaction engine, and the two load-bearing gate scripts) out of
ziee's `src-app/ui/src/dev/gallery/` + `scripts/` into a new
`sdk/packages/gallery` (`@ziee/gallery`), mirroring the `sdk/packages/{kit,
framework}` src-export convention. ziee's OWN surfaces stay app-side: the 36
`modules/*/gallery.tsx`, the `fixtures/` cassettes, the hand `coverage.ts` /
`stateCoverage.ts`, the `.generated.ts` artifacts, allowlists, `stories/`, and
demos — **all unchanged**. The 36 per-module galleries import from
`@/dev/gallery/support` **byte-unchanged**; the seam is a thin app-side binding
barrel.

## Design-gate — the `Cassette<T>` compile-time check SURVIVES the boundary (TOP RISK)

`Cassette` was `{ [K in ApiEndpoint]?: GetResponseType<K> | resolver }`, typed
against ziee's generated `@/api-client/types` so a wrong cassette shape fails
`tsc`. The package can't import the app's generated types, so `Cassette` +
`ModuleGallery` became **generic over the app's response map**
(`Cassette<TResponses>` / `ModuleGallery<TResponses>`), and the app binds the
concrete alias once (`src/dev/gallery/support/types.ts`:
`type Cassette = GCassette<ApiEndpointResponses>`). The check is preserved at the
authoring site — **PROVEN**: a scratch `const bad: ModuleGallery = { cassette: {
'Assistant.list': 12345 } }` fails with
`error TS2322: Type 'number' is not assignable to type 'CassetteEntry<AssistantListResponse>'`.
(MOVE: `mockApi.ts:40-47`, `support/types.ts:85` → `sdk/packages/gallery/src/mock/mockApi.ts` + `registry/types.ts`.)

## Design-gate — `import.meta.glob` discovery STAYS app-side, injected

`import.meta.glob` is Vite-only and cannot cross a package boundary. The eager
glob (`support/registry.ts:47`, `../../../modules/**/gallery.{ts,tsx}`) stays in
ziee; the discovered `DiscoveredGallery[]` is INJECTED into the framework via
`mountGallery({ discoverGalleries })`. The pure merge/assert
(`mergeModuleCassettes` / `assertUniqueSlugs`) MOVED to the package
(`registry/registry-core.ts`); eager, synchronous assembly ordering is preserved
(`mountGallery` calls `initSurfaces(discoverGalleries())` BEFORE `installMockApi`).

## Design-gate — every `@/` reach becomes a `GalleryConfig` field (DI)

The 12 former `@/` couplings (api-client route table, `@/modules/{loader,router,
auth}`, `@/components/{ThemeProvider,AppErrorBoundary}`, `@/core/components/
{Loading,LazyComponentRenderer}`, accent tokens, config-store theme writers,
`@/index.css`, the SSE/PDF/text special routes, the chat `ConversationPage` deep
frame, the `providerId` param, `ALL_STORIES`) all become injected `GalleryConfig`
fields, assembled in the one app-side `src/dev/gallery/galleryConfig.ts`.
`@ziee/kit` + `@ziee/framework/stores` stay hard package deps.

## Design-gate — prod-exclusion marker is a FIXED literal (`ZIEE_GALLERY_SEED_MARKER`)

The `data-gallery-build-marker="ZIEE_GALLERY_SEED_MARKER"` JSX literal (the string
`check-gallery-prod-exclusion.mjs` greps for) is emitted as a FIXED literal by the
package `GalleryPage.tsx`. `gallery.config.json`'s `prodMarker` MUST equal it
(defaults to it). Proven present at runtime (smoke read the attribute).

## Design-gate — NO Rust / OpenAPI / generated-`types.ts` impact

Pure frontend. `git status` shows zero changes to any `api-client/types.ts`,
`openapi.json`, or Rust file. E8 trivially byte-identical.

## MOVES (framework files → `sdk/packages/gallery/src/`)

| ziee source (deleted / reduced to shim) | → package dest |
|---|---|
| `mockApi.ts` (engine) | `mock/mockApi.ts` (generic `Cassette<T>` + `configureMockApi` + `SpecialRoute` DI) |
| `mockApi-binary.ts` (+`.test.ts`) | `mock/mockApi-binary.ts` (+ `.test.ts`) |
| `support/registry-core.ts` (+`registry.test.ts`) | `registry/registry-core.ts` (+ `.test.ts`) |
| `support/types.ts` (ModuleGallery contract) | `registry/types.ts` (generic `ModuleGallery<T>`) |
| `pages.tsx` + `overlays.tsx` + `deepStates.tsx` + `seededSurfaces.tsx` (frames) | `runtime/pages.tsx` (all four frames, DI'd) + `runtime/surfaces-registry.ts` |
| `surfaces.ts` | `runtime/surfaces.ts` |
| `GalleryPage.tsx` (ControlBar + canvas) | `runtime/GalleryPage.tsx` |
| `main.tsx` + `seed.ts` (boot) | `runtime/mount.tsx` (`mountGallery`) |
| `matrix.ts` | `runtime/matrix.ts` (accent = generic `string`) |
| `useGalleryTheme.ts` | `runtime/useGalleryTheme.ts` |
| `story.tsx` | `runtime/story.tsx` |
| `interactions.ts` | `runtime/interactions.ts` (verbatim) |
| `support/lazy.tsx` + `support/hold.ts` (+`index.test.ts`) | `runtime/lazy.tsx` + `runtime/hold.ts` (+ `hold.test.ts`) |
| `scripts/gate-ui.mjs` | `scripts/gate-ui.mjs` (config-driven) |
| `scripts/runtime-health.mjs` | `scripts/runtime-health.mjs` (config-driven) |
| `scripts/lib/gallery-surfaces.mjs` | `scripts/lib/gallery-surfaces.mjs` (verbatim) |
| `plugins/vite-plugin-gallery-{alias,coverage}.js` | `vite/vite-plugin-gallery-{alias,coverage}.js` (copies) |

NEW in package: `runtime/config.ts` (`GalleryConfig` + `getGalleryConfig`),
`runtime/mount.tsx` (`mountGallery`), `runtime/surfaces-registry.ts`,
`scripts/lib/gallery-config.mjs` (the resolver), `scripts/cli.mjs`, `src/index.ts`.

## STAYS app-side (ziee content, unchanged)

`modules/*/gallery.tsx` (×36), `fixtures/**`, `coverage.ts`, `stateCoverage.ts`,
`*.generated.ts`, `*-allowlist.json`, `*.md` manifests, `stories/**`, the demos
(`DefectRepro`/`TableDemos`/`*LongDemo`), `runtime-baseline.js`, and the
content-coupled scripts (gen-crawl-cassette, check-gallery-fixtures,
capture-forms, record-gallery-fixtures). App-side thin shims retained at their
old paths so ziee content imports are byte-unchanged: `mockApi.ts`, `story.tsx`,
`GalleryPage.tsx`, `support/{index,types,registry,registry-core}.ts` +
NEW `galleryConfig.ts` + `main.tsx` (mountGallery boot) + `gallery.config.json`.

`gen-override-registry.mjs` is the desktop-override system — left OUT of this
extraction (belongs to `@ziee/framework/overrides`), per the audit.
