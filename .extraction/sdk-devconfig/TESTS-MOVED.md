# Chunk sdk-devconfig — TESTS

The extracted artifacts are LINTS/GENERATORS (their "tests" are the lints running
against a real tree). Nothing test-wise was relocated out of ziee; a new SDK smoke
test covers the package surface, and the lints themselves serve as the executable proof.

## New (SDK)
- **T-CFG-1** [added→sdk] `sdk/packages/config/scripts/config.test.mjs` — `biome.base.json`
  is valid JSON with the generic preset AND has no app-specific `noRestrictedImports`.
- **T-CFG-2** [added→sdk] same file — `tsconfig.base.json` exposes strict generic
  compilerOptions AND no app `paths`.
- **T-CFG-3** [added→sdk] same file — `defineSyncpack` composes semver + version groups
  with the catch-all `sameRange` group LAST.
- **T-CFG-4** [added→sdk] same file — **parameterization proof**: `hardcoded-colors.mjs`
  scans an arbitrary `--root` dir; a clean `.tsx` passes (exit 0) and a `bg-blue-500`
  `.tsx` fails (exit 1, message names the token). → 4/4 PASS.

## Executable equivalence proofs (the backward-compat anchor)
- The 5 extracted lints (colors/settings-field/adjacent-inline/logical-direction/
  tooltip-placement), run from `src-app/ui` against ziee's roots, emit stdout **diffed
  byte-identical** to the pre-change baseline. (exit 0 each.)
- `design-spec --check` → `up to date`; generating to a temp file diffs byte-identical
  against the committed `DESIGN_SYSTEM.md`.
- `kit-manifest --check` against the SDK kit barrel → `up to date` (67 components) —
  fixes the pre-existing break.
- `biome check ./src` (extends) → identical counts + exit; `lint:guardrails` identical;
  `tsc --noEmit` (ui + desktop) exit 0; `syncpack lint` byte-identical.

## Regression evidence
- ziee `ui/` `tsc --noEmit` — exit 0 (== baseline).
- ziee `desktop/ui/` `tsc --noEmit` — exit 0 (== baseline, untouched).
- All 8 ziee config-subset `check` steps PASS (`check:kit-manifest` improved broken→pass).
- SDK `@ziee/config` smoke — 4/4 PASS.
