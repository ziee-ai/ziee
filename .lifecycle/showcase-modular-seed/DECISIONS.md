# DECISIONS — showcase-modular-seed (Phase 4)

Every human/product + implementation input is resolved up front — zero unresolved
markers remain. No ITEM is descoped (DEC-A = full migration), so there are no
`DESCOPED:` dispositions.

### DEC-1: Migration scope — full migration now, or mechanism+gaps first?
**Resolution:** FULL migration — all 155 central entries move into per-module
`gallery.tsx`; the 4 central files become pure auto-discovering aggregators.
**Basis:** user (AskUserQuestion 2026-07-13, "Full migration now").

### DEC-2: Desktop workspace — include this round, or follow-up?
**Resolution:** Include. Desktop gallery aggregators glob shared + desktop-only
module `gallery.tsx`; seed the 5 desktop-only modules; run the gate in the desktop
workspace.
**Basis:** user (AskUserQuestion 2026-07-13, "Include desktop now").

### DEC-3: New gap seed data — hand-authored typed literals, or recorded?
**Resolution:** Hand-authored typed literals (typed vs `@/api-client`, tsc-checked;
contract-tested vs `openapi.json`). `record-gallery-fixtures.mjs` untouched.
**Basis:** user (AskUserQuestion 2026-07-13, "Hand-authored typed literals").

### DEC-4: Per-module seed file path + name?
**Resolution:** `src/modules/<X>/gallery.tsx`, exporting `export const gallery: ModuleGallery`.
**Basis:** convention — mirrors `module.tsx` co-location + the `import.meta.glob('./**/module.tsx')` discovery in `modules/loader.ts`.

### DEC-5: Must a crawl-covered module have a file, or can it be allow-listed?
**Resolution:** Every surface-bearing module MUST have a `gallery.tsx`; a module fully
covered by the shared crawl exports `{ crawlOnly: true }` (a conscious ownership
marker). The allow-list (`GALLERY_SEED_EXCEPTIONS.md`) is ONLY for the 5 INFRA-ONLY
modules (config-client, layouts, router, settings) + any genuinely surfaceless one.
**Basis:** convention — uniform per-module ownership; the allow-list mirrors
`OVERRIDE_EXCEPTIONS.md` (reason + sign-off, structural-only).

### DEC-6: How does the gate decide "user-facing surface" (plain Node, no vite)?
**Resolution:** parse `module.tsx`: TRUE if it declares a route `path:` literal not in
`{'/','/dev/gallery','/auth/callback'}`, OR registers a user-facing slot in the curated
set `{settingsUserPages, settingsAdminPages, sidebarNavigation, sidebarContent,
sidebarBottom, sidebarFooter, sidebarTools, sidebarPrimaryActions, appBanners,
registerPanelRenderer}`. Pure exported `hasUserSurface(src)` unit-tested.
**Basis:** codebase — these are the actual route/slot registration idioms (survey/04
slot map); mirrors how `gen-override-registry.mjs` regexes source without vite.

### DEC-7: Where do gallery-local demo components + cross-cutting kit surfaces live?
**Resolution:** the `dev-gallery` module owns them — `src/modules/dev-gallery/gallery.tsx`
holds `DefectRepro`, `TableDemos` (×8 seeded), `MessageListLongDemo`, and the two
`components/ui` dialog-host overlays. So `dev-gallery` gets a real `gallery.tsx` (NOT
allow-listed). Kit-component *stories* (`stories/`) stay central (design-system, not
module seed).
**Basis:** convention — the gallery's own module is the natural home for gallery-local
+ kit-cross-cutting surfaces; keeps every entry owned by SOME module.

### DEC-8: Manifest + exceptions file locations?
**Resolution:** committed `src/dev/gallery/GALLERY_SEED_MANIFEST.md` (generated,
byte-compared) + `src/dev/gallery/GALLERY_SEED_EXCEPTIONS.md` (hand, sign-off). Both
permanent product-tree paths.
**Basis:** convention (B6) — mirrors `core/overrides/OVERRIDE_MANIFEST.md` +
`desktop/ui/OVERRIDE_EXCEPTIONS.md`; never `.lifecycle/`.

### DEC-9: Cross-module cassette-key collision behavior?
**Resolution:** `mergeModuleCassettes` THROWS (dev-only) on a duplicate endpoint key
across two module galleries — surfaces the ambiguity at gallery boot.
**Basis:** convention — fail-loud over silent last-wins; the crawl base is exempt
(always overridden by module entries).

### DEC-10: Desktop gate scope — does it double-count shared modules?
**Resolution:** the desktop `check:gallery-seed-registry` scans ONLY
`desktop/ui/src/modules/**` (desktop-only modules). Shared modules' seed lives in
`ui/src/modules` and is enforced by the ui workspace's gate.
**Basis:** convention — each workspace gates its own module tree; avoids double
enforcement + a false-missing on shared modules the desktop tree doesn't contain.

### DEC-11: TEST-9 migration baseline — how is "no surface lost" anchored?
**Resolution:** capture the CURRENT (pre-migration) slug set from the running gallery
into a committed fixture `tests/e2e/visual/__fixtures__/gallery-seed-baseline.json`
BEFORE the refactor; TEST-9 asserts the post-migration `listAllSurfaces()` ⊇ baseline.
**Basis:** convention — a golden baseline is the standard regression anchor; captured
once at the start of Phase 5.

### DEC-12: Does this feature introduce any operational tunable (settings row)?
**Resolution:** NO. It is a dev-only visual-test harness — no resource limit,
retention, quota, toggle, or model selection reaches runtime/production. No
`settings` table, migration, or permission. The Phase-4 configurable-settings rule is
satisfied by explicit N/A.
**Basis:** convention — the gallery + gate are `import.meta.env.DEV` / build-time only;
nothing operator-facing.

### DEC-13: Prod-exclusion enforcement mechanism?
**Resolution:** a unique sentinel comment (`ZIEE_GALLERY_SEED_MARKER`) in the registry;
`check-gallery-prod-exclusion.mjs` runs `vite build` and asserts the marker is absent
from the app entry's chunks. On leak: gate the `dev-gallery` module's lazy
`import('@/dev/gallery/GalleryPage')` behind `import.meta.env.DEV` so the reference is
dropped in prod.
**Basis:** codebase — prod build has no `rollupOptions.input` for `gallery.html`
(verified); this makes the invariant machine-checked, not assumed.
