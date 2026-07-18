# Chunk `sdk-testinfra` — BOUNDARY

- E1 (CUT present, ≥1 move line): PASS — CUT.md with a full source→dest MOVE table
  (8 generators/captures/coverage/prod-exclusion + 2 vite-plugin dup removals) +
  the additive test-e2e table + 5 design-gates.
- E2 (TRANSFORMS: every differing symbol has a T-N; Decisions; no TBD): PASS —
  T-1..T-15 + D-1..D-5, all RESOLVED, zero TBD.
- E3 (LEDGER valid, ≥8 angles, incl equivalence + security): PASS — 12 entries,
  12 distinct angles incl. `equivalence` (ti-01) + `security` (ti-03).
- E4 (AUDIT_COVERAGE: every diff hunk reconciled, ≥3 angles): PASS — every sdk +
  ziee changed path (incl. deletions) has a row with ≥3 angles.
- E5 (move-completeness: every dest exists; every symbol resolves): PASS — every
  MOVE dest exists under `packages/gallery/scripts/*` + `playwright/*`; the 5
  test-e2e src files + barrel exist; `@ziee/test-e2e` + `@ziee/gallery` tsc = 0
  resolve every symbol; `@ziee/gallery/vite/*` imports resolve to functions.
- E6 (source-deletion: moved generic engine absent from ziee as a divergent dup):
  PASS — the 8 moved generators + 2 vite-plugin dups are `git rm`ed from ziee;
  ziee references the package via repointed npm scripts + vite.config imports.
  **Exception (declared):** `lib/gallery-surfaces.mjs` stays in ziee (still used by
  the NOT-moved `affordance-audit` + `gen-crop-review`) — a pre-existing package/ziee
  dup from the prior `sdk-gallery` chunk, out of scope here.
- E7 (transform-declared: every differing moved symbol has a T-N): PASS (T-1..T-8
  for the moves; T-10..T-15 for the new package surfaces).
- E8 (regen-parity / golden): PASS — `git status` shows ZERO changes to any
  `api-client/types.ts` / `openapi.json` / `.rs` / migration / `.sql`; the moved
  generators reproduce their gallery artifacts byte-identically, so those on-disk
  generated files are unchanged too.
- E9 (clean-build): PASS — test-e2e tsc = 0; gallery tsc = 0; visual.config.ts
  tsc = 0; ziee `ui` tsc = 0 + `desktop/ui` tsc = 0; 8 moved .mjs + cli + config
  pass `node --check`; 17 relocated unit tests pass.
- E10 (no divergent duplicate / dead code): PASS — each moved generic generator
  exists ONCE (package). No dead config (the provisional `testidOut` field was
  removed with the testid deferral).
- E11 (seam-purity): PASS — `@ziee/test-e2e` names only its own `E2EConfig` seam +
  `@playwright/test` (peer) + lazy `pg`/`dotenv`; zero `@/` imports. The gallery
  generators name only `./lib/gallery-config.mjs` + the app's `gallery.config.json`.
- E12 (submodule-pin): PASS — sdk committed LOCALLY at
  `0174b83ab674725dd2b6844a47111c71c64a00cc` (branch `sdk-testinfra`, NOT pushed);
  ziee records the new pointer (staged). pgvector submodule NOT touched/staged.

## Equivalence run

- **Byte-parity (moved generators)**: ziee-original vs package write-mode diff —
  `stateMatrix.generated.ts` / `STATE_MATRIX.md` / `galleryCoverage.generated.ts` /
  `overlay-registry.generated.json` ALL IDENTICAL.
- **seed-registry `--check`**: PASS from `ui/` (39 modules) AND `desktop/ui/ --src
  src` (9 modules) — via the repointed package path.
- **unit tests**: `node --test` on the 2 relocated files = 17 pass / 0 fail;
  `npm run test:gallery-seed-registry` (ui, repointed) = 17 pass.
- **vite repoint boot smoke**: gallery Vite dev server serves `/gallery.html` +
  the alias-rewritten `/dev-gallery.html`, no load errors.
- **golden (openapi + types.ts)**: IDENTICAL (untouched).

## Scope boundary — declared follow-ups (NOT regressions)

- **gen-testid-registry (DEFERRED — kit-migration follow-up).** The testid
  registry is mid-migration into the kit package: the committed
  `sdk/packages/kit/src/testIds.generated.ts` (1590 ids) already includes
  kit-package-src ids, but ziee's `ui/scripts/gen-testid-registry.mjs` still writes
  the DELETED `src/components/ui/testIds.generated.ts` path and walks only
  ui+desktop (1588). Cleanly parameterizing it requires resolving that migration
  (adding the kit tree to the walk + repointing OUT + regenerating the committed
  file), which is the kit workstream's job. Per the STOP-rule it's left app-side
  UNCHANGED (ziee behaves identically); moving it is a follow-up bundled with the
  kit testid migration.

- **B3 (full desktop gallery rewire — DEFERRED, needs a desktop host).** Desktop
  keeps its OWN local copies of `gen-overlay-registry` / `gen-gallery-coverage` /
  `gen-state-matrix` / the captures / `gate-ui` / `runtime-health` /
  `lib/gallery-surfaces` under `desktop/ui/scripts/` (untouched). Folding those in
  + a desktop `mountGallery`/config boot needs a desktop host to verify the gallery
  actually renders; it stays a desktop-host follow-up (as the prior `sdk-gallery`
  BOUNDARY already noted). Only desktop's shared cross-refs to files THIS chunk
  moved (`gen-gallery-seed-registry`) were repointed — a safe, Linux-verifiable
  package.json path flip, NOT the render rewire.

- **ziee `playwright.visual.config.ts` NOT repointed.** The package ships
  `defineVisualConfig` (B2), but ziee keeps its own app-side visual config (the
  gate reads it as `CFG.visualConfig`) to avoid touching the visual gate. Repoint
  (`export default defineVisualConfig()`) is a trivial, user-verifiable flip —
  a B4-style follow-up, deliberately left out to keep the gate green.

- **`gen-override-registry.mjs` OUT of scope** (belongs to the desktop-override
  system → `@ziee/framework/overrides`), per the task.

- **`@ziee/test-e2e` is a SCAFFOLD, not a ziee repoint.** ziee's e2e keeps its own
  richer `global-setup`/`fixtures` (per-worker vite+backend port pairs, deep
  readiness gate). Migrating ziee's suite ONTO the package presets is a separate,
  opt-in follow-up; this chunk ships the reusable scaffold + proves it tsc-clean.
