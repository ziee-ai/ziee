# INFRA_INTEGRATION — the two mandatory walks (Phase 5)

## UX walk — who uses this and how
The "user" is a coding agent / reviewer running the visual-test system, and any
engineer adding a module. Their end-to-end encounter:
1. Adds a module → creates `src/modules/<X>/module.tsx` with a route. Runs
   `npm run check` → the new `check:gallery-seed-registry` FAILS with
   "module <X> has a user-facing surface but no src/modules/<X>/gallery.tsx".
2. Creates `src/modules/<X>/gallery.tsx` exporting `gallery` (cassette + any
   overlays/seeded). Re-runs check → green; the gallery now renders their module
   populated at `/gallery.html?surface=<X>`.
3. Reviewer opens the gallery / runs `gate:ui` → every module's surfaces render
   across states; regressions (crash/console-error/contrast) are caught.
This closes the current hole where 5 modules render empty and nobody is alerted.

## Infrastructure-integration walk — every subsystem touched
- **Vite `import.meta.glob` + bundling.** Eager glob assembles the cassette before
  `installMockApi`. RISK: eager glob leaks per-module `gallery.tsx` into any chunk
  importing the registry → prod bloat. CONSTRAINT: registry/aggregators imported
  ONLY from the dev-gallery chunk (main.tsx / DEV-gated dev-gallery route). Guarded
  by ITEM-16 (`check-gallery-prod-exclusion` + real `vite build`). Prod build has no
  `gallery.html` input (verified) so it is excluded by default.
- **The module system (`modules/loader.ts`).** The app loader globs `module.tsx`
  ONLY; `gallery.tsx` is invisible to it (no double-registration, no prod pull-in).
  The gallery registry globs `gallery.tsx` ONLY. Clean separation — a module's
  `module.tsx` must NOT import its `gallery.tsx`.
- **Existing gallery gates.** `gen-overlay-registry` path-couples to `overlays.tsx`
  (ITEM-18 broadens it). `gen-gallery-coverage` + `gen-state-matrix` are
  source-driven (walk `modules`/`components/ui`) — UNAFFECTED, but they key on
  surface ids + slugs, so migration MUST preserve every slug (baseline: 155,
  committed `gallery-seed-baseline.json`).
- **`surfaces.ts::listAllSurfaces` + capture/coverage/runtime-health.** Consume the
  exported arrays (`OVERLAY_ENTRIES`/`DEEP_STATE_ENTRIES`/`SEEDED_SURFACE_ENTRIES`) +
  the browse DOM. Aggregators keep the SAME exports → these tools need zero change.
- **`seed.ts` ordering.** `installMockApi(GALLERY_CASSETTE)` must precede
  `loadModules()`. Eager glob keeps assembly synchronous at module-eval → invariant
  preserved.
- **Desktop override plugin + parallel gallery.** Desktop `@/` resolves desktop-first
  then falls back to `../../ui/src`. Desktop gallery aggregators glob BOTH shared
  (`../../../../ui/src/modules/**`) + desktop-only (`../../modules/**`). Desktop gate
  scans desktop-only tree only (DEC-10). R2-3: keep desktop gallery structurally in
  sync with ui (pure dev harness — no security logic).
- **Playwright / runtime-health.** New surfaces (5 modules + 3 overlays + gaps) get
  gallery cells → must pass runtime-health (0 HIGH). New e2e specs live under
  `tests/e2e/visual/` (the existing visual home).
- **Permissions / chat / MCP / sync / settings / notifications.** NOT touched — no
  runtime behavior, no backend, no permission, no settings row (DEC-12). The seeded
  surfaces are the modules' REAL components rendered offline; no product code changes.
- **`record-gallery-fixtures.mjs` + crawl.** Untouched (shared base). New seed is
  hand-authored typed literals (DEC-3), validated by the existing
  `check-gallery-fixtures` contract test vs `openapi.json`.
