# FIX_ROUND-1 — blind-audit findings + resolutions (Phase 7)

Phase-6 blind audit ran 4 fresh diff-only agents across the angle roster (findings
in `LEDGER.jsonl` / `audit/ledger-{1..4}.jsonl`; coverage in `AUDIT_COVERAGE.tsv`,
every hunk ≥3 angles). Confirmed findings + how each was fixed:

## HIGH
- **prod-exclusion was vacuous AND the gallery was leaking to prod**
  (`check-gallery-prod-exclusion.mjs`). The sentinel lived only in a JSDoc comment,
  which minifiers strip — so the check could never fail. Worse, when made a REAL
  runtime marker it immediately caught that the dev-gallery `import()` reference
  (a top-level `const GalleryPage = lazyWithPreload(...)`) kept the gallery
  reachable, so Rollup shipped it as a lazy `GalleryPage` chunk (+ the mock-cassette
  data) in prod. Fixes: (1) emit a runtime `data-gallery-build-marker="ZIEE_GALLERY_SEED_MARKER"`
  on `gallery-root` (a used JSX string literal — survives minification, is NOT a
  `data-test-*` attr so the prod stripper leaves it); (2) move the lazy `import()`
  INSIDE the `import.meta.env.DEV` branch in `modules/dev-gallery/module.tsx` so the
  reference is dead code in prod → Rollup drops the gallery entirely (verified: prod
  bundle 548→491 JS assets, no `GalleryPage` chunk, marker absent); (3) the check
  `rmSync(dist)` before building (vite doesn't empty an out-of-root `outDir`).

## MEDIUM
- **`isMain` guard broke on Windows** (`gen-gallery-seed-registry.mjs`,
  `gen-overlay-registry.mjs`). `import.meta.url === \`file://${argv[1]}\`` is false on
  Windows/spaced paths → the gate body is skipped → `--check` silently exits 0.
  Fixed to `pathToFileURL(process.argv[1]).href` (the portable form already used in
  `gallery-geometry-audit.mjs`).
- **e2e "renders populated" assertions were vacuous** (`gallery-newly-seeded.spec.ts`,
  `gallery-gap-seed.spec.ts`, desktop `gallery-desktop-seed.spec.ts`).
  `expect(text.length).toBeGreaterThan(40)` read the `gallery-page-<slug>` SECTION,
  which always carries ~50 chars of gallery chrome — so an empty seed would pass.
  Fixed to measure the `[data-gallery-frame]` rendered-component subtree (>20 chars).
  Re-run: all pass, so the surfaces genuinely render populated.
- **desktop TEST-17 shared assertion proved nothing** (`gallery-desktop-seed.spec.ts`).
  `settings-users` is populated by `User.list`, already in the desktop CRAWL base —
  so it passed even if the cross-workspace `SHARED_CASSETTE` were broken. Fixed to
  `settings-js-tool` (`JsTool.getSettings` comes ONLY from the web `js-tool/gallery.tsx`
  cassette, absent from the crawl) → genuinely proves cross-workspace inheritance.
  Also aligned the desktop `PageFrame` markers (`data-gallery-frame`/`-chrome`) with ui.

## LOW (robustness)
- Removed redundant inline `Auth.getSessionSettings` (the `...authCassette` spread
  already seeds it) in `auth/gallery.tsx`.
- Removed the dead `OVERLAY_ENTRIES` export from desktop `module-seed.ts` (the desktop
  gallery is page-focused; it never renders shared overlays — cassette-only inheritance).
- `extractWiredSurfaces` now matches `surface: "…"` (double-quote) too.
- `parseSeedExceptions` now requires a CLOSED `[approved:…]` (an unterminated token no
  longer counts as a sign-off).
- `gallery.ts` (not just `.tsx`) excluded consistently across the coverage + state-matrix
  gates in both workspaces.
- `hasUserSurface` already gained comment-stripping in Phase 5 (window's commented
  route false-positive).

## Rejected / accepted-as-is (with rationale)
- `enumerateModules` reads only `module.tsx` (a split-out routes file would escape the
  gate): ACCEPTED — routes are declared in `module.tsx` by hard convention across all
  39 modules; a future split would be the anomaly to fix then.
- `SeededSurfaceFrame` uses `fallback={()=>null}` (crash marker ineffective for the
  seeded class): PRE-EXISTING, out of this diff's scope; the 3 new specs target
  enumerated pages/overlays that DO emit the crash marker.
- `app/gallery.tsx` `needs_setup:true` in the global cassette: benign — pages render in
  isolated MemoryRouters without the app root's `routerEffects`, so no redirect fires.
- desktop `SHARED_CASSETTE as Cassette` cast: safe — web components render web data via
  the `@/` fallback; extra web-only keys are never matched by the desktop client.

**New confirmed findings:** 0
