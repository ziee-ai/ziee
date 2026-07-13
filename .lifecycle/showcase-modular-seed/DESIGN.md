# DESIGN — per-module showcase seed + completeness gate

## Goal
Each module OWNS its gallery seed in a co-located file, auto-discovered like
`module.tsx`, so adding/seeding a module never touches central gallery files; a
committed gate fails if a surface-bearing module has no registered seed.

## 1. The per-module contract — `src/modules/<X>/gallery.tsx`

A module opts into the gallery by exporting a `gallery` object (mirrors how it
`export default createModule(...)` in `module.tsx`). New shared type
`ModuleGallery` in `src/dev/gallery/support/types.ts`:

```ts
export interface ModuleGallery {
  /** Mock-API seed BEYOND the shared crawl base: detail-route resolvers,
   *  query-keyed resolvers, mutations, richer list data. Typed `Cassette`. */
  cassette?: Cassette
  /** Overlay open-states (was overlays.tsx OVERLAY_ENTRIES). */
  overlays?: OverlayEntry[]
  /** Deep active-conversation states (was deepStates.tsx). */
  deepStates?: DeepStateEntry[]
  /** Real-component + store-seed surfaces (was seededSurfaces/shard*). */
  seeded?: SeededSurfaceEntry[]
  /** Kit-component stories (rare; most stay central in stories/). */
  stories?: GalleryStory[]
  /** Explicit "this module's pages render fully from the shared crawl base;
   *  no bespoke seed needed" — a conscious marker, not an omission. Lets a
   *  crawl-covered module satisfy the gate without empty arrays. */
  crawlOnly?: true
}
```

Per-module `gallery.tsx` imports helpers from a shared **`src/dev/gallery/support/`**
barrel (the generalized `seeded/helpers.tsx`: `lazyNamed`/`lazyBound`/`lazyProps`/
`lazyCompose`/`holdPatch`/`holdForever`/`whenTrue` + the entry types). It imports
the module's own components/stores lazily. It is referenced ONLY by the gallery
entry's glob — never by `module.tsx` — so it is tree-shaken out of the prod app
build (same dev-only story the gallery already has).

## 2. Runtime discovery (vite) — central files become thin aggregators

`import.meta.glob` runs in the gallery entry (vite), so the central files collapse
to auto-discovering aggregators. One shared collector
`src/dev/gallery/support/registry.ts`:

```ts
// Eager so the cassette is fully assembled BEFORE installMockApi/loadModules.
const mods = import.meta.glob<{ gallery?: ModuleGallery }>(
  '../../../modules/**/gallery.{ts,tsx}', { eager: true },
)
export const MODULE_GALLERIES = Object.entries(mods)
  .map(([path, m]) => ({ module: moduleNameFromPath(path), g: m.gallery }))
  .filter(x => x.g)
```

- `fixtures/index.ts`: `GALLERY_CASSETTE = { ...crawlCassette, ...mergeModuleCassettes(MODULE_GALLERIES) }`
  (crawl base first; per-module entries win; a **duplicate endpoint key across two
  modules throws** at assemble time — cross-module collisions are a bug).
- `overlays.tsx` → `OVERLAY_ENTRIES = MODULE_GALLERIES.flatMap(x => x.g.overlays ?? [])`.
- `deepStates.tsx` / `seededSurfaces.tsx` → same flatten. Slug-uniqueness asserted
  at assemble (dev-throws on collision), replacing the shard slug-prefix convention.
- `surfaces.ts::listAllSurfaces()` is unchanged — it already reads these arrays +
  the browse DOM, so every downstream tool (capture/coverage/runtime-health) keeps
  working with zero changes.

**Ordering constraint preserved:** the glob is eager + synchronous, so the full
cassette exists before `seed.ts` calls `installMockApi()` → `loadModules()`.

**Crawl cassette stays shared infra** (recorded from a real server; not per-module
authorship). Per-module `gallery.tsx` only ADDS beyond it. `record-gallery-fixtures.mjs`
and the crawl gates are untouched.

## 3. The completeness gate — `scripts/gen-gallery-seed-registry.mjs`

Mirrors `gen-override-registry.mjs` exactly (set-difference + committed allow-list +
byte-compared manifest), runs in plain Node (fs + regex, no vite):

- **MODULES** = subdirs of `src/modules/`.
- **HAS_SURFACE(m)** — parse `m/module.tsx`: true if it declares a route `path:`
  literal not in `{'/','/dev/gallery','/auth/callback'}`, OR registers a user-facing
  slot (curated key set: `settingsUserPages`/`settingsAdminPages`/`sidebarNavigation`/
  `sidebarContent`/`sidebarBottom`/`sidebarFooter`/`sidebarTools`/`sidebarPrimaryActions`/
  `appBanners` + `registerPanelRenderer`). Exported pure fn, unit-tested against
  fixtures.
- **HAS_SEED(m)** — `m/gallery.ts|tsx` exists AND matches `export const gallery`.
- **ALLOWLIST** — committed `src/dev/gallery/GALLERY_SEED_EXCEPTIONS.md`, lines
  `- NO-SEED: <module> — <reason> [approved: <who/when>]` (parsed like
  OVERRIDE_EXCEPTIONS.md; requires reason + sign-off). Only for the 5 INFRA-ONLY
  modules.
- **MISSING** = `{ m ∈ HAS_SURFACE : ¬HAS_SEED(m) ∧ m ∉ ALLOWLIST }` → non-empty ⇒
  `exit(1)` with a per-module remediation message.
- **STALE_ALLOWLIST** = `{ allowlisted m : HAS_SEED(m) ∨ ¬HAS_SURFACE(m) }` → `exit(1)`
  (GC, so the excuse list can't rot).
- Emits committed `src/dev/gallery/GALLERY_SEED_MANIFEST.md` (living index: module →
  surface kinds → seed file → status), byte-compared in `--check`.
- Pure exported `computeSeedDrift(...)` → unit-testable.
- Wire `gen:gallery-seed-registry` + `check:gallery-seed-registry` (`--check`),
  append `&& npm run check:gallery-seed-registry` LAST in `npm run check`.

**B6 (survives merge strip):** every input is a permanent product-tree path
(`src/modules/**/module.tsx`, `src/modules/**/gallery.tsx`,
`src/dev/gallery/GALLERY_SEED_EXCEPTIONS.md`, `GALLERY_SEED_MANIFEST.md`) — never
`.lifecycle/`. Verified against a lifecycle-stripped tree before done.

## 4. Migration (Goal 2 — seed every module)

Move the existing 155 central entries into their owning module's `gallery.tsx`
(mechanical — the entry objects are already module-scoped; see the slug→module
tables in `survey/03`). The 7 hand fixtures move into their modules' `cassette`.
Then author NEW seed for the real gaps:
- **5 UNSEEDED:** `js-tool`, `knowledge-base`, `notification`, `scheduler`, `voice`
  — cassette entries for their on-load GETs (+ list/detail data), wire their 3
  unwired overlays (KnowledgeBaseFormDrawer / ScheduledTaskFormDrawer /
  UploadModelDrawer), + a `kb_source` deep/seeded panel for knowledge-base.
- **Gaps:** `app.getSetupStatus`, `onboarding` guide steps, `auth` session-settings,
  `code-sandbox.listRootfsVersions`, `file.get` for `/files/:fileId`.

New cassette data is either recorded (extend `record-gallery-fixtures.mjs` arms) or
hand-authored typed literals (both already-supported channels).

## 5. Desktop parity
Desktop gallery aggregators glob BOTH the shared `../../../../ui/src/modules/**/gallery.tsx`
(inherits every shared module's seed for free) AND desktop-only
`../../modules/**/gallery.tsx`. Seed the desktop-only surface-bearing modules
(host-mount, remote-access, tunnel-auth, updater, window) with their own
`gallery.tsx`. The completeness gate runs in the desktop workspace too (globs the
desktop module tree; shared modules are covered by the ui workspace's gate). Both
`npm run check` stay green.

## Non-goals / preserved
- The shared crawl cassette + `record-gallery-fixtures.mjs` recording flow — kept.
- `surfaces.ts`, `pages.tsx` enumeration, `interactions.ts`, capture/coverage tools —
  unchanged (they read the same arrays).
- Per-surface gates (`gallery-coverage`, `state-matrix`, `overlay-registry`) — kept;
  the new gate is a coarser PER-MODULE guard layered above them, not a replacement.
- No backend changes, no migration, no OpenAPI regen, no new permission.
