# Chunk `sdk-gallery` — BOUNDARY

- E1 (CUT present, ≥1 move: line, Design-gate): PASS — CUT.md, 4 design-gates + a
  MOVE table with source→dest lines.
- E2 (TRANSFORMS: every differing symbol has a T-N; Decision Resolution; no TBD):
  PASS — T-1..T-10 + D-1..D-4 all RESOLVED, zero TBD.
- E3 (LEDGER valid, ≥8 angles, includes equivalence + security): PASS — 12
  entries, 12 distinct angles incl. `equivalence` (gal-02) + `security` (gal-03).
- E4 (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS — see
  AUDIT_COVERAGE.tsv (package src + app shims + scripts, ≥3 angles each).
- E5 (move-completeness: every move dest exists in SDK; every Symbol resolves):
  PASS — every framework file in the CUT MOVE table exists under
  `sdk/packages/gallery/src/` (+ `scripts/` + `vite/`); package tsc = 0 resolves
  every symbol.
- E6 (source-deletion: moved generic engine absent from ziee as a divergent
  duplicate): PASS — the moved framework files are `git rm`-deleted from ziee
  (mockApi-binary, registry-core, pages, overlays, deepStates, seededSurfaces,
  surfaces, matrix, useGalleryTheme, interactions, seed, support/lazy, support/
  hold). The retained same-path files are thin SHIMS (re-export the package +
  bind the app's api-client types) — the equivalence mechanism, not duplicates.
  **Exception (declared):** the desktop workspace's local framework copies
  (`desktop/ui/src/dev/gallery/{main,GalleryPage,pages,mockApi,matrix,surfaces,
  seed,useGalleryTheme,overlays,story}.tsx`) are RETAINED unchanged — see B-3.
- E7 (transform-declared: every differing moved symbol has a T-N): PASS
  (T-1..T-10).
- E8 (regen-parity / golden): PASS — pure frontend; `git status` shows ZERO
  changes to any `api-client/types.ts`, `openapi.json`, or Rust file → the 4
  generated files are BYTE-IDENTICAL, no regen.
- E9 (clean-build): PASS — `@ziee/gallery` tsc = 0; ziee `ui` tsc = 0; `desktop/ui`
  tsc = 0; `node --test` on the 3 relocated unit files = 11 pass / 0 fail.
- E10 (no divergent duplicate / dead code): PASS — the generic engine exists ONCE
  (in the package); ziee's retained files reference it. (Desktop's local copies
  are a known pre-existing duplicate the extraction did not yet fold in — B-3.)
- E11 (seam-purity / SDK names only the seam): PASS — `@ziee/gallery` has ZERO
  `@/` imports (`grep -rn "@/" sdk/packages/gallery/src` = 0); it names only its
  own DI seam (`GalleryConfig`) + `@ziee/kit` / `@ziee/framework/stores`.
- E12 (submodule-pin): sdk submodule committed LOCALLY (no push); ziee records the
  new pointer (staged). pgvector submodule NOT touched/staged.

## Equivalence run

- Standalone gallery smoke (Playwright, live vite): 43 pages / 47 overlays / 17
  deep / 99 seeded enumerated via the injected glob; 72 story sections; auth page
  + an overlay render with 0 console errors; marker present. (LEDGER gal-02.)
- Config-driven package `runtime-health.mjs` (run from `ui/`, scoped `--only-match=hub`
  against the live gallery): loaded `gallery.config.json` + the runtimeBaselineModule,
  audited 7/7 cells, hub surface clean (0 gating HIGH). (LEDGER gal-09.)
- golden(openapi + types.ts): IDENTICAL (untouched).

## Scope boundary — declared follow-ups (NOT regressions; everything green)

- **B-1 (remaining config-drivable scripts)** — the 5 registry generators
  (`gen-testid`/`gen-state-matrix`/`gen-gallery-coverage`/`gen-overlay-registry`/
  `gen-gallery-seed-registry`), the 2 captures, `gallery-coverage.mjs`, and
  `check-gallery-prod-exclusion.mjs` remain as ziee's WORKING app-side scripts
  (untouched, identical). Re-homing them under `@ziee/gallery/scripts/*` (they are
  config-drivable per the audit §4) is the next wave. The load-bearing gates
  (`gate-ui` + `runtime-health`) + the resolver + `lib/gallery-surfaces` + the 2
  vite plugins ARE re-homed here and ziee's `gate:ui`/`gallery:runtime` npm scripts
  repointed to them.
- **B-2 (playwright templates)** — `playwright.visual.config.ts` + the generic
  spec templates (`layout`/`states`/`overlays`/`gallery`) stay app-side; shipping
  them as `@ziee/gallery/playwright/*` templates is deferred (heavily baseline-/
  content-coupled per audit §1b).
- **B-3 (full desktop rewire)** — desktop's local framework copies compile +
  behave identically via the `registry-core` re-export shim (D-3/FIX-1.3).
  Deleting them + adding a desktop `galleryConfig.ts` + `mountGallery` boot (with
  a merging `discoverGalleries` from `module-seed.ts`, page-focused so web overlay
  entries stay excluded) is the declared follow-up. Desktop is GREEN meanwhile.
- **B-4 (repoint ziee's other npm scripts + vite.config plugins)** — ziee keeps
  its own plugin copies wired in `vite.config.ts` (untouched, to avoid dev-server
  risk); the package ships copies for a minimal app. Repointing is a trivial,
  user-verifiable flip.

`gen-override-registry.mjs` is intentionally OUT of scope (desktop-override system
→ `@ziee/framework/overrides`).
