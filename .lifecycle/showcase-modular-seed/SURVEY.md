# SURVEY — how the dev gallery seeds surfaces today (Phase 1)

Detailed findings in `survey/01..05`. This is the consolidated picture.

## The gallery in one paragraph
`src-app/ui/src/dev/gallery/` renders every real page/overlay/component across
states, seeded by a backend-free **mock-API cassette** (`window.fetch`
interceptor answering `/api/*` from a typed `Cassette` map). It is served
standalone at `/gallery.html` (dev-only) and drives the whole visual-test system
(`gate:ui`, `runtime-health`, Layer A/B). The desktop workspace has a parallel
copy at `src-app/desktop/ui/src/dev/gallery/`.

## Four surface classes (`surfaces.ts::listAllSurfaces()`)
| Class | Count | Registered where | Rendered how | Seeded how |
|---|---|---|---|---|
| **pages** | ~40 routes | AUTO — enumerated at render from the router store (`pages.tsx::useResolvedPages`) | real route in a `MemoryRouter` | populated ONLY if the cassette answers its on-load GET |
| **overlays** | 44 | central `overlays.tsx` `OVERLAY_ENTRIES[]` | real Drawer/Dialog, portaled | store `open()` action or bound props |
| **deep** | 17 | central `deepStates.tsx` `DEEP_STATE_ENTRIES[]` (chat-only) | shared `ConversationPage` pinned to a `conversationId` | store `setState` after `whenLoaded` |
| **seeded** | 94 | central `seededSurfaces.tsx` + `seeded/shard1..5.tsx` | real component + router path | store `setState` (`holdPatch`/`holdForever`/`whenTrue`) or bound props |

**Key mechanic:** overlays/deep/seeded do NOT use per-entry cassettes — they
render the REAL component + REAL store and layer the transient/failure state via
`setState`. The cassette answers GETs; entries add what a GET-only harness can't
reach.

## The cassette (the "seed") — fully CENTRALIZED
- Assembled in ONE file `fixtures/index.ts`: `crawl.generated.ts` (60 recorded
  **param-less GET** endpoints, shared base) overlaid last-wins by 7 hand-authored
  module fixtures (auth, chat, citations, llm-providers, project-deep, workflow,
  skills).
- `Cassette = { [K in ApiEndpoint]?: GetResponseType<K> | ((ctx)=>...) }` — typed
  against the generated api-client, so a wrong shape fails `tsc`. Entry forms:
  literal, query-keyed resolver, path-param resolver.
- Recorded by `scripts/record-gallery-fixtures.mjs` (boots a real server) →
  validated vs `openapi.json` by `check-gallery-fixtures` + `check:gallery-crawl`.
- Unrecorded endpoints → a crash-safe `makeSafeEmpty()` array-proxy (page renders
  empty, never crashes).

## Existing gates in `npm run check` (all plain Node, no vite)
- `check:gallery-coverage` — every surface id (471, fs-walked `.tsx`) must appear in
  `coverage.ts` with its kind's REQUIRED_STATES; `kind: pending|static|nonvisual|
  flow|via` is the inline exclusion.
- `check:state-matrix` — ts-morph extracts per-surface render states → a generated
  `RequiredState` union; `stateCoverage.ts` must cover each (`{via}`/`{skip,reason}`).
- `check:overlay-registry` — fs-walk finds kit overlay primitives; a "host" overlay
  must be wired in `overlays.tsx` or allowlisted in `overlay-allowlist.json` (+ GC of
  stale allowlist entries).
- `check:override-registry` — **THE PRECEDENT**: set-difference gate over the desktop
  override tree; reads a committed allow-list `desktop/ui/OVERRIDE_EXCEPTIONS.md`
  (`- SHADOW-EXCEPTION: <path> — <reason> [approved: <who/when>]`); B6-compliant.

## Meta-framework module auto-discovery (the pattern to mirror)
- `src/modules/loader.ts`: `import.meta.glob('./**/module.tsx', {eager:true})` →
  each module `export default createModule({...})`.
- Extension fields ride via **declaration merging**: `router/types.ts` adds
  `routes?` to `CreateModuleOptions`; `createModule` spreads `...options`; the router
  module's `onModuleRegister` hook harvests `module.routes` from every module.
- `import.meta.glob` is available in the gallery entry (vite) but NOT in the Node
  check scripts (they fs-walk / ts-morph instead) — the two-channel split the design
  must respect.

## Module census roll-up (39 modules — see `survey/04`)
- **UNSEEDED (5, real gaps):** `js-tool`, `knowledge-base`, `notification`,
  `scheduler`, `voice`. Their on-load GETs aren't in the crawl → pages render
  empty/crash. Three also ship an **unwired overlay**
  (KnowledgeBaseFormDrawer / ScheduledTaskFormDrawer / UploadModelDrawer).
- **PARTIAL / SEEDED-with-a-gap:** `app` (getSetupStatus), `onboarding` (guide steps),
  `auth` (`/settings/sessions`), `code-sandbox` (`listRootfsVersions`),
  `file` (`File.get` for `/files/:fileId`).
- **SEEDED (24):** the rest, mostly via crawl + central overlays/seeded; only ~7 have
  rich hand-authored fixtures.
- **INFRA-ONLY (5):** config-client, dev-gallery, layouts, router, settings.
- **Desktop-only modules (need seed in the desktop gallery):** host-mount,
  remote-access, tunnel-auth, updater, window (+ desktop overrides of memory/layouts).

## The problem, precisely
Not "nothing is seeded" — but **all seed authoring is centralized** (`fixtures/index.ts`,
`overlays.tsx`, `deepStates.tsx`, `seededSurfaces.tsx`). Adding/seeding a module means
editing 1-4 central files; there is **no per-module ownership** (the seeded *shard*
contract is the only partial precedent) and **no gate** forcing a surface-bearing
module to have seed — so the 5 unseeded modules slipped through silently.
