# Chunk `sdk-gallery-b3` — BOUNDARY

- E1 (CUT present, ≥1 move line): PASS — CUT.md with a full desktop-rewire
  DELETE/replace table + the testid-generator MOVE row + 5 design-gates.
- E2 (TRANSFORMS: every differing symbol has a T-N; Decisions; no TBD): PASS —
  T-1..T-9 + D-1..D-4, all RESOLVED, zero TBD.
- E3 (LEDGER valid, ≥8 angles, incl equivalence + security): PASS — 12 entries,
  12 distinct angles incl. `equivalence` (gb3-01) + `security` (gb3-11).
- E4 (AUDIT_COVERAGE: every changed path reconciled, ≥3 angles): PASS — every sdk
  + ziee changed path (incl. the 10 deletions) has a row with ≥2–3 angles.
- E5 (move-completeness: every dest exists; every symbol resolves): PASS — the
  moved generator + its test exist under `packages/gallery/scripts/`; the desktop
  binding files exist; tsc = 0 (×3) resolves every symbol.
- E6 (source-deletion: moved generic engine absent from ziee as a divergent dup):
  PASS — the 8 desktop framework copies + the old testid generator + the web
  `registry-core` shim are `git rm`'d; the framework exists ONCE (the package).
- E7 (transform-declared: every differing moved symbol has a T-N): PASS (T-1..T-9).
- E8 (regen-parity / golden): PASS — `git status` shows ZERO changes to any
  `api-client/types.ts` / `openapi.json` / `.rs` / migration / `.sql`; the kit
  `testIds.generated.ts` is byte-identical to committed (the union already matched).
- E9 (clean-build): PASS — `@ziee/gallery` tsc = 0; ziee `ui` tsc = 0; `desktop/ui`
  tsc = 0; `node --test` on the 3 gallery-script test files = 21 pass / 0 fail;
  biome guardrails clean on ui + desktop; desktop `check:gallery-seed-registry` OK.
- E10 (no divergent duplicate / dead code): PASS — framework engine ONCE (package);
  desktop holds thin bindings; `GALLERY_CASSETTE` (dead post-rewire) removed;
  `MODULE_CASSETTE` retained is the live input to `discoverGalleries`.
- E11 (seam-purity / SDK names only the seam): PASS — the moved generator names
  only `resolveGalleryConfig` fields (`srcDir`/`extraTrees`/`kitTestIds`/`testidOut`),
  zero `@/` imports, zero ziee paths baked in.
- E12 (submodule-pin): sdk committed LOCALLY on branch `sdk-gallery-b3` (NOT
  pushed); ziee records the new pointer (staged). `vendor/pgvector` NOT touched/staged.

## Equivalence run

- **testid byte-parity**: moved generator (write mode, both cwds) reproduces
  `sdk/packages/kit/src/testIds.generated.ts` (1590 ids) with a ZERO diff;
  `--check` PASSES from `ui/` AND `desktop/ui/`.
- **desktop gallery render-through-package (xvfb)**: 14 e2e specs pass (2 seed + 6
  override + 6 runtime render/axe), zero console/page error, axe clean, light+dark;
  the rendered `gallery-root` carries the PACKAGE build-marker
  `ZIEE_GALLERY_SEED_MARKER`; 50 surfaces enumerate; runtime-health A7 canary = 0
  gating HIGH.
- **golden (openapi + types.ts)**: IDENTICAL (untouched).

## Scope boundary — declared follow-ups (NOT regressions; everything green)

- **Desktop's OTHER local gallery scripts stay app-side (from the sdk-testinfra
  B3 deferral).** `desktop/ui/scripts/{gen-overlay-registry,gen-gallery-coverage,
  gen-state-matrix,gate-ui,runtime-health,capture-*,affordance-audit,
  gallery-geometry-audit}.mjs` are UNCHANGED. This chunk repointed only the two
  config-drivable, cwd-independent generators desktop shares
  (`gen-gallery-seed-registry` — already done in sdk-testinfra — and now
  `gen-testid-registry`). Re-homing the desktop-local capture/coverage/gate
  scripts onto the package is a separate B-follow-up (they read desktop-local
  baselines + the desktop `runtime-baseline.js`).
- **`playwright.gallery.config.ts` stays app-side** — the desktop gallery specs
  keep their own config (self-boots the Vite server); no package template repoint
  (mirrors the web `playwright.visual.config.ts` deferral).
- **ziee's `playwright.visual.config.ts` still NOT repointed** (unchanged from
  sdk-testinfra) — orthogonal to this chunk.
- **The moved generator's emitted header string is kept verbatim** ("… across the
  ui + desktop trees") so the committed kit registry is byte-unchanged; the
  accurate provenance (it now also unions kit/shell) lives in the source comments.
  Correcting the emitted string is a cosmetic future flip (would rewrite the
  committed file for zero functional gain).
