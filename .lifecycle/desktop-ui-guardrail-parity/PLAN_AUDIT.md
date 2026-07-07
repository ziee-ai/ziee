# PLAN_AUDIT — desktop-ui-guardrail-parity

Audit of PLAN.md against the actual codebase (read before writing code).

## Breakage risk

- Adding gates to desktop `npm run check` will make `check` NEWLY FAIL wherever
  desktop-own (or shared) source violates a gate that was never run on desktop.
  This is the intended discovery mechanism, but it couples the gate-wiring items
  (ITEM-1/6) to the fix items (ITEM-9..11): Phase 8's
  `npm run check (desktop/ui): PASS` line requires those violations resolved
  first. Concretely at risk:
  - `check:overlay-registry` (`gen-overlay-registry.mjs --check`) **exits 1 on any
    MISSING surface** — an overlay host (Dialog/Drawer) found under
    `desktop/ui/src/modules` but not registered in `overlays.tsx` / allowlist
    (verified: `gen-overlay-registry.mjs:232-238,280`). Desktop-only overlays
    (updater dialog, file-dialog, any confirm) must be registered or allowlisted.
  - `lint:icon-action` / `lint:adjacent-inline` scan desktop/ui/src (dual-root) and
    may surface desktop-only violations → fixed under ITEM-11/ITEM-9.
  - `check:testid-registry` regenerates the SHARED server-ui
    `testIds.generated.ts` from BOTH roots; if desktop source introduced a testid
    not yet in the committed registry it fails — must regen + commit.
- The reuse gates (`check:kit-manifest`, `check:design-spec`) target server-ui /
  shared artifacts already kept green by the merged audit, so they are low-risk
  no-ops for desktop that simply add guard parity.
- Copying audit SCRIPTS (ITEM-2..6) adds no runtime/product code — dev tooling
  only; zero caller breakage.

## Pattern conformance

- ITEM-1 mirrors the exact `../../ui/scripts/<x>.mjs` reference desktop already
  uses for `lint:colors` etc. — conformant.
- ITEM-2..6 copies mirror desktop's existing self-owned gallery-relative copies
  (`gen-state-matrix.mjs`, `gen-gallery-coverage.mjs`, `gen-crawl-cassette.mjs`),
  which resolve via `__dirname` and auto-point at the desktop gallery — conformant.
- ITEM-6/7 overlays manifest mirrors server-ui `overlays.tsx` +
  `overlay-registry.generated.json` shape — conformant.
- **Correction (impl note):** `gen-crop-review-manifests.mjs` reads the taxonomy at
  `path.resolve(UI_DIR,'docs/DEFECT_TAXONOMY.md')` (verified line 37), NOT the
  gallery dir. So ITEM-5 must place the file at
  `src-app/desktop/ui/docs/DEFECT_TAXONOMY.md` (matching server-ui `ui/docs/`),
  not the gallery. PLAN "Files to touch" note is amended accordingly here.
- tsc scope: desktop `tsconfig.json` include is `["src","tests","../../ui/src"]`
  (verified) — `scripts/*.mjs` and `docs/*.md` are OUT of tsc scope, so the copied
  `.mjs` files never enter type-checking. The new `overlays.tsx` IS under `src` →
  it is type-checked (must compile).

## Migration collisions

- None. This feature adds NO SQL migrations and touches NO `migrations/` files.
  `ls migrations/` is irrelevant to this diff.

## OpenAPI regen

- None required. The diff touches NO backend Rust types, NO
  `#[derive(JsonSchema)]`, NO route response shapes. No `openapi.json` /
  `api-client/types.ts` regeneration in either workspace. `check:testid-registry`
  regenerates `testIds.generated.ts`, but that is the testid registry (not the
  OpenAPI client) and is committed as a normal source artifact.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure package.json wiring reusing dual-root/shared
  scripts already proven on server-ui; matches desktop's existing `../../ui/scripts`
  pattern.
- **ITEM-2** — verdict: PASS — `gallery-geometry-audit.mjs` resolves via `__dirname`
  → a desktop copy auto-targets the desktop gallery; empty allowlist is the
  documented default (script creates/reads `geometry-allowlist.json`).
- **ITEM-3** — verdict: PASS — `affordance-audit.mjs` + allowlist are gallery-relative
  copies; no cross-workspace assumption.
- **ITEM-4** — verdict: CONCERN — `detector-acceptance.mjs` runs sibling `scripts/*`
  against `src/dev/gallery/__detector_fixtures__`; requires copying BOTH the script
  and the fixtures, and desktop must also have the sibling detector scripts it
  invokes (geometry/icon-action/native-scroll). Resolve: copy fixtures + ensure the
  invoked detectors exist in desktop scripts (geometry via ITEM-2; icon-action is
  referenced from `../../ui/scripts` — confirm detector-acceptance can point at it,
  else copy). Verified in Phase 4 DEC-2.
- **ITEM-5** — verdict: CONCERN — taxonomy path is `docs/DEFECT_TAXONOMY.md`, not the
  gallery (see Pattern correction). Place the copy at `desktop/ui/docs/`.
- **ITEM-6** — verdict: CONCERN — `--check` FAILS on unregistered desktop overlays
  (verified 232-238). Must enumerate desktop-only Dialog/Drawer hosts and register
  each in `overlays.tsx` or allowlist BEFORE the gate can be green. Bounded by the
  small desktop-only module set.
- **ITEM-7** — verdict: PASS — additive gallery story/overlay entries rendered via
  the existing mock-API cassette; mirrors server-ui gallery story wiring.
- **ITEM-8** — verdict: PASS — audit RUN; produces an out-of-tree findings ledger,
  no product-code risk.
- **ITEM-9** — verdict: PASS — geometry fixes are token/spacing edits on
  desktop-only surfaces guided by the detectors + taxonomy; standard UI work.
- **ITEM-10** — verdict: PASS — contrast / a11y-name / console-error fixes on
  desktop-only surfaces, gated by the desktop runtime-health pass.
- **ITEM-11** — verdict: PASS — affordance fixes (accessible names, tap targets)
  on desktop-only surfaces.
