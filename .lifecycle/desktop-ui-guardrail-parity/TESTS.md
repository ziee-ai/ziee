# TESTS — desktop-ui-guardrail-parity

Every ITEM maps to ≥1 TEST. UI diff → ≥1 `tier: e2e`. Mock only external
boundaries; the gate commands exercise the real tooling against the real desktop
gallery ([[feedback_no_cosmetic_tests]]).

## Guardrail-backfill tests (ITEM-1..6)

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/desktop/ui/src/dev/guardrails/guardrail-parity.test.ts` — asserts: desktop `package.json` `check` chains `lint:adjacent-inline`, `lint:icon-action`, `check:kit-manifest`, `check:testid-registry`, `check:design-spec`; each referenced `../../ui/scripts/*.mjs` path resolves on disk.
- **TEST-2** (tier: unit) [covers: ITEM-2] file: `src-app/desktop/ui/src/dev/guardrails/guardrail-parity.test.ts` — asserts: `scripts/gallery-geometry-audit.mjs` exists in desktop and is byte-identical to the web source; `package.json` defines `gallery:geometry` + `gallery:geometry:gate`; `src/dev/gallery/geometry-allowlist.json` exists and parses.
- **TEST-3** (tier: unit) [covers: ITEM-3] file: `src-app/desktop/ui/src/dev/guardrails/guardrail-parity.test.ts` — asserts: `scripts/affordance-audit.mjs` + `scripts/affordance-audit-allowlist.json` exist; `package.json` defines `gallery:affordance`.
- **TEST-4** (tier: unit) [covers: ITEM-4] file: `src-app/desktop/ui/src/dev/guardrails/detector-acceptance.test.ts` — asserts: `scripts/detector-acceptance.mjs`, `scripts/lint-icon-action.mjs`, `scripts/lint-native-scroll.mjs`, and `src/dev/gallery/__detector_fixtures__/` exist; running `node scripts/detector-acceptance.mjs` exits 0 — the two LINT detectors FIRE on the copied fixtures AND the desktop geometry detector is byte-identical to the web source (drift guard). No dev server required.
- **TEST-5** (tier: unit) [covers: ITEM-5] file: `src-app/desktop/ui/src/dev/guardrails/guardrail-parity.test.ts` — asserts: `scripts/gen-crop-review-manifests.mjs` + `docs/DEFECT_TAXONOMY.md` exist; `package.json` defines `gen:crop-review`; the script reads `docs/DEFECT_TAXONOMY.md` and the taxonomy carries the `[V]` vision-rubric lines the crop pass parses. (Full manifest generation runs in the ITEM-8 audit pass — it needs a live gallery server.)
- **TEST-6** (tier: unit) [covers: ITEM-6] file: `src-app/desktop/ui/src/dev/guardrails/overlay-registry.test.ts` — asserts: `scripts/gen-overlay-registry.mjs`, `src/dev/gallery/overlays.tsx`, `overlay-allowlist.json`, and committed `overlay-registry.generated.json` exist; `check:overlay-registry` is chained into `check`; `node scripts/gen-overlay-registry.mjs --check` exits 0 (every desktop-only Dialog/Drawer host registered or allowlisted).

## Composite static gate (ITEM-1,6)

- **TEST-7** (tier: integration) [covers: ITEM-1, ITEM-6] file: `src-app/desktop/ui/package.json` — asserts: `npm run check` in `src-app/desktop/ui` exits 0 with the full backfilled gate chain (tsc + all lints + kit/testid/design/overlay/gallery/state checks) — recorded as the `npm run check (desktop/ui): PASS` gate line.

## Audit + fix tests (ITEM-7..11)

- **TEST-8** (tier: e2e) [covers: ITEM-7] file: `src-app/desktop/ui/tests/e2e/gallery-desktop-runtime.spec.ts` — asserts: the desktop-only ROUTE chrome surfaces the gallery CAN render (`settings-about`, `settings-remote-access`, `settings-host-mount`) render with NO `gallery-crash` boundary in light AND dark. (Reality per DRIFT-1.3: window/file-dialog/desktop-base have no DOM; UpdateBanner/app-layout are Tauri/shell-coupled — verified DOM-less/coupled, not gallery-added.)
- **TEST-9** (tier: e2e) [covers: ITEM-8, ITEM-10] file: `src-app/desktop/ui/tests/e2e/gallery-desktop-runtime.spec.ts` — asserts: the desktop-only route surfaces report ZERO console/page-error in the LOADED state across light+dark (proves the F1 build-break is gone and no runtime crash) — mirrors the `gate:ui` runtime-health roll-up (0 HIGH).
- **TEST-10** (tier: e2e) [covers: ITEM-9] file: `src-app/desktop/ui/tests/e2e/gallery-desktop-runtime.spec.ts` — asserts: `node scripts/gallery-geometry-audit.mjs --gate --surfaces=settings-about,settings-remote-access,settings-host-mount` (desktop) exits 0 — no unresolved (un-allowlisted HIGH) geometry findings on the desktop-only surfaces.
- **TEST-11** (tier: e2e) [covers: ITEM-11] file: `src-app/desktop/ui/tests/e2e/gallery-desktop-runtime.spec.ts` — asserts: `node scripts/affordance-audit.mjs --report-only` reports 0 gating affordance misses on the desktop surfaces (the affordance gate is green).
- **TEST-12** (tier: e2e) [covers: ITEM-10] file: `src-app/desktop/ui/tests/e2e/gallery-desktop-runtime.spec.ts` — asserts: the desktop-only route surfaces pass axe a11y (no serious/critical violation; every interactive control has an accessible name), across light+dark.
