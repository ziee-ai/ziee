# TESTS — showcase-modular-seed (Phase 3)

Bipartite: every ITEM (1-18) is covered by ≥1 TEST; every TEST names a valid ITEM,
tier, file, assertion. No new permission is introduced → no `[negative-perm]` spec
required. The diff touches both frontend workspaces (`src-app/ui`, `src-app/desktop/ui`)
→ ≥1 `tier: e2e` enumerated (TEST-9..13, 17). Pure logic lives in exported functions
so unit tests need no vite; render/behavior claims are proven by e2e against the real
gallery (B7 — verify by running).

## Unit — the completeness gate (pure fns)
- **TEST-1** (tier: unit) [covers: ITEM-7] file: `src-app/ui/scripts/gen-gallery-seed-registry.test.mjs` — asserts: `computeSeedDrift` returns the surface-bearing-but-seedless module in MISSING; returns empty MISSING when every surface module has a `gallery.tsx`.
- **TEST-2** (tier: unit) [covers: ITEM-7, ITEM-14] file: `src-app/ui/scripts/gen-gallery-seed-registry.test.mjs` — asserts: STALE_ALLOWLIST flags an allow-listed module that now HAS seed OR has no user surface (GC), failing the gate.
- **TEST-3** (tier: unit) [covers: ITEM-7] file: `src-app/ui/scripts/gen-gallery-seed-registry.test.mjs` — asserts: `hasUserSurface` is true for a non-skip route `path:` and for a user-facing slot key, false for a module with only skip-paths (`/`,`/dev/gallery`,`/auth/callback`) or no route/slot.
- **TEST-4** (tier: unit) [covers: ITEM-7, ITEM-14] file: `src-app/ui/scripts/gen-gallery-seed-registry.test.mjs` — asserts: the exceptions parser accepts `- NO-SEED: <m> — <reason> [approved: <who/when>]` and rejects a line missing the reason or the `[approved:…]` sign-off.
- **TEST-15** (tier: unit) [covers: ITEM-17] file: `src-app/ui/scripts/gen-gallery-seed-registry.test.mjs` — asserts: a module whose `gallery.tsx` exports only `{ crawlOnly: true }` counts as HAS_SEED (not MISSING).
- **TEST-5** (tier: unit) [covers: ITEM-18] file: `src-app/ui/scripts/gen-overlay-registry.test.mjs` — asserts: `getWiredSurfaces()` picks up a `surface:'x'` literal declared in a per-module `gallery.tsx` fixture, not only in `overlays.tsx`.

## Unit — the runtime registry (pure fns, no glob)
- **TEST-6** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/ui/src/dev/gallery/support/registry.test.ts` — asserts: `mergeModuleCassettes` overlays a module entry over the crawl base for the same key, AND THROWS when two module galleries seed the same endpoint key.
- **TEST-7** (tier: unit) [covers: ITEM-2] file: `src-app/ui/src/dev/gallery/support/registry.test.ts` — asserts: `assertUniqueSlugs` throws on a duplicate slug across overlays/deep/seeded and passes on distinct slugs.
- **TEST-8** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/dev/gallery/support/index.test.ts` — asserts: the `support/` barrel re-exports `holdPatch`/`holdForever`/`whenTrue`/`lazyNamed`/`lazyBound`/`lazyProps` and a well-formed `ModuleGallery` fixture type-checks (compile) + round-trips its arrays.

## Integration — build + check
- **TEST-14** (tier: integration) [covers: ITEM-16] file: `src-app/ui/scripts/check-gallery-prod-exclusion.mjs` — asserts: after `vite build`, no chunk reachable from the app `index.html` entry contains the per-module gallery sentinel string (`ZIEE_GALLERY_SEED_MARKER`) — proving `gallery.tsx` is dev-only, never in the prod app bundle.
- **TEST-20** (tier: integration) [covers: ITEM-7, ITEM-14] file: `src-app/ui/package.json` (`npm run check`) — asserts: full `npm run check` (ui) passes INCLUDING `check:gallery-seed-registry`, with the 5 INFRA-ONLY modules allow-listed (the gate green end-to-end on the real tree).
- **TEST-16** (tier: integration) [covers: ITEM-15] file: `src-app/desktop/ui/package.json` (`npm run check`) — asserts: desktop `npm run check` passes including `check:gallery-seed-registry` over the desktop module tree (shared modules covered by the ui gate; desktop-only modules seeded).

## E2E — the real gallery (Playwright)
- **TEST-9** (tier: e2e) [covers: ITEM-3, ITEM-4, ITEM-5, ITEM-6, ITEM-8, ITEM-9, ITEM-10, ITEM-11] file: `src-app/ui/tests/e2e/visual/gallery-seed-parity.spec.ts` — asserts: on `/gallery.html`, `window.__GALLERY_LIST_ALL_SURFACES__()` returns a slug set that is a SUPERSET of a committed pre-migration baseline (no migrated overlay/deep/seeded surface lost) with no duplicate slug.
- **TEST-10** (tier: e2e) [covers: ITEM-12] file: `src-app/ui/tests/e2e/visual/gallery-newly-seeded.spec.ts` — asserts: for each of `settings-js-tool`, `knowledge`, `notifications`, `scheduled-tasks`, `settings-voice`, the `?surface=<slug>` render shows non-empty content, no `[data-testid=gallery-crash]`, and no console error.
- **TEST-11** (tier: e2e) [covers: ITEM-12] file: `src-app/ui/tests/e2e/visual/gallery-newly-seeded.spec.ts` — asserts: the 3 newly-wired overlays (`overlay-knowledge-base-form-drawer`, `overlay-scheduled-task-form-drawer`, `overlay-upload-model-drawer`) render OPEN with populated form content.
- **TEST-12** (tier: e2e) [covers: ITEM-13] file: `src-app/ui/tests/e2e/visual/gallery-gap-seed.spec.ts` — asserts: the gap surfaces render populated — app SetupPage (getSetupStatus), `/settings/sessions` (session settings), code-sandbox rootfs section (listRootfsVersions), `/files/:fileId` viewer (File.get) — each non-empty, no crash.
- **TEST-13** (tier: e2e) [covers: ITEM-2, ITEM-4, ITEM-5, ITEM-6, ITEM-8, ITEM-9, ITEM-10, ITEM-11] file: `src-app/ui/tests/e2e/visual/gallery-runtime-health.spec.ts` — asserts: driving `runtime-health` over the migrated + newly-seeded surfaces yields ZERO HIGH findings (console-error/page-error/nav-error/request-failed/contrast) in light + dark.
- **TEST-17** (tier: e2e) [covers: ITEM-15] file: `src-app/desktop/ui/tests/e2e/visual/desktop-gallery-seed.spec.ts` — asserts: the desktop gallery boots and the 5 desktop-only module surfaces (host-mount, remote-access, tunnel-auth magic, updater about, window prefs) render populated, no crash, no console error.

## Required gate lines (Phase 8 — recorded in TEST_RESULTS.md)
- `npm run check (ui): PASS` · `npm run check (desktop/ui): PASS`
- `gate:ui (ui): PASS` · `gate:ui (desktop/ui): PASS` (A7 boot/runtime canary)

## ITEM → TEST coverage map (self-check)
ITEM-1→8 · 2→6,7,13 · 3→6,9,20 · 4→9,13 · 5→9,13 · 6→9,13 · 7→1,2,3,4,15,20 ·
8→9,13 · 9→9,11,13 · 10→9,13 · 11→9,13 · 12→10,11 · 13→12 · 14→2,4,20 · 15→16,17 ·
16→14 · 17→15 · 18→5. Every ITEM covered; every TEST maps to a live ITEM.
