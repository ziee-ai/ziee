# BASE.md — conflict-surface scoping (P3)

Feature: **showcase-modular-seed** — modularize the dev gallery so each module owns
its showcase seed, add a completeness gate, and seed every module.

## Base commit
- Branch `feat/showcase-modular-seed` cut from `origin/main` @ `482a9cd05`
  (Merge PR #142). `git log origin/main -1` == branch base at plan time.

## Highest existing migration
- `src-app/server/migrations/00000000000157_remove_unused_builtin_mcp_servers.sql`
- **This feature adds ZERO migrations** — it is frontend-only (dev gallery + build
  scripts). No migration-number collision possible.

## OpenAPI regen implied?
- **No.** No backend types change. The cassette fixtures are TYPED against the
  EXISTING generated `@/api-client/types` (`GetResponseType<K>`); they consume the
  spec, they do not change it. No `just openapi-regen` needed. (If a module's
  page needs an endpoint that does not yet exist in the spec, that is a signal to
  stop, not to regen — the seed only mirrors real endpoints.)

## Files CURRENT main also touches (collision watch)
- The dev-gallery tree (`src-app/ui/src/dev/gallery/**`) is under active churn:
  recent main commits touched it (`feat/desktop-ui-override` raw-shadow gate,
  `fix/streaming-empty-notice-flicker`, Users-group composer trim). Risk is
  **moderate** because this feature ADDS per-module seed files + refactors the
  central assembly points (`fixtures/index.ts`, `overlays.tsx`, `deepStates.tsx`,
  `seededSurfaces.tsx`, `surfaces.ts`). Mitigation: keep the central files as thin
  auto-discovering aggregators (small diff surface) and put the bulk of new code
  in NEW per-module files (`src/modules/*/gallery.*`) that main is not touching.
- `package.json` (ui + desktop/ui) `check` script — appends ONE new `check:*` step
  (append-only, low collision).
- `src-app/desktop/ui/src/dev/gallery/**` — the parallel desktop gallery copy; same
  refactor mirrored. Desktop has its own subset `fixtures/` + `loadDesktopModules`.

## Load-bearing precedents this branch mirrors
- Module auto-discovery: `import.meta.glob('./**/module.tsx', {eager:true})` in
  `src/modules/loader.ts`; declaration-merge extension field (`routes?` on
  `CreateModuleOptions` in `src/modules/router/types.ts`) harvested by an
  `onModuleRegister` hook.
- Gate precedent: `scripts/gen-override-registry.mjs` (`--check`) + the committed
  allow-list `desktop/ui/OVERRIDE_EXCEPTIONS.md`
  (`- SHADOW-EXCEPTION: <path> — <reason> [approved: <who/when>]`), wired last in
  `npm run check`. B6-compliant (reads a PERMANENT product-tree path, not
  `.lifecycle/`).

## Baseline gate state (untouched tree, verified green at plan time)
- `check:gallery-coverage` PASS · `check:overlay-registry` PASS ·
  `check:override-registry` PASS · `check:state-matrix` PASS
  (365 surfaces / 2150 signals / 374 required-state keys / 3 panels).
