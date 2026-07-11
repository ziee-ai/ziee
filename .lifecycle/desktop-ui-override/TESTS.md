# TESTS.md — enumerated tests (bipartite: every ITEM ↔ ≥1 TEST)

No new permission is introduced (build/render infrastructure), so no A9
backend-deny / A10 `[negative-perm]` e2e is required. UI paths ARE touched, so
`tier: e2e` tests are enumerated (TEST-9, TEST-10, TEST-11).

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/core/overrides/registry.test.ts` — asserts: `registerOverride` then `resolveOverride` returns the component; unregistered key → `undefined`; re-register is last-wins and observable.
- **TEST-2** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/ui/src/core/overrides/seam.test.tsx` — asserts: `<Seam id props>{fallback}</Seam>` renders the fallback children when nothing is registered and renders override(props) when registered; `useOverride(key, Fallback)` behaves identically; props forward to whichever wins. (`keyof UIOverrides` type enforcement proven by `tsc` in `npm run check`.)
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/desktop/ui/src/modules/desktop-base/overrides.test.ts` — asserts: invoking the desktop registration entry registers exactly the expected seam keys (each `resolveOverride(key)` is then defined) with no DOM access (safe at pre-render init time).
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/desktop/ui/plugins/local-override.test.ts` — asserts: the extracted pure resolver picks, in order, desktop-tree `foo.*` → core-tree `foo.desktop.*` → core-tree `foo.*` over a fixture tree, and returns null on no match; plus a case proving a `foo.desktop.tsx` is treated as a shadow (not a duplicate) by the testid-scan helper.
- **TEST-5** (tier: unit) [covers: ITEM-7] file: `src-app/ui/scripts/seam-codemod.test.mjs` — asserts: on fixture shadow+core pairs, `migrate` rewrites core to wrap the diverging element in `<Seam>`, generates the `UIOverrides` declaration, emits the desktop `registerOverride`, and removes the shadow; `add <key>` scaffolds decl + registration stub + manifest row. Deterministic output compared to golden fixtures.
- **TEST-6** (tier: unit) [covers: ITEM-9] file: `src-app/desktop/ui/src/modules/layouts/app-layout/components/Drawer.test.ts` — asserts: the desktop Drawer's restored stacking-guard decision (`isHigherLayerPresent`) — a layer stacked strictly above triggers the guard, the drawer itself never counts, equal/lower/non-finite-z layers don't. Proves the guard the desktop shadow had dropped is ported back (full DOM behavior via TEST-9 e2e).
- **TEST-7** (tier: unit) [covers: ITEM-10, ITEM-12] file: `src-app/ui/scripts/gen-override-registry.test.mjs` — asserts: the generator emits a manifest listing all declared seams (the ITEM-12 doc index); `--check` PASSES on a matched set and FAILS (non-zero) on (a) an override for an undeclared key (dead override) and (b) an orphaned `*.desktop.tsx` with no core sibling.
- **TEST-8** (tier: unit) [covers: ITEM-8] file: `src-app/desktop/ui/src/modules/desktop-base/seam-parity.test.tsx` — asserts: for each converted seam (HardwareMonitorButton) — with the desktop registration applied the seam resolves to the desktop variant, and without it renders the core fallback; locks the behavior of the deleted shadow into the new mechanism.
- **TEST-9** (tier: e2e) [covers: ITEM-6, ITEM-8] file: `src-app/desktop/ui/tests/e2e/desktop/desktop-override-seam.spec.ts` — asserts: booting the DESKTOP build, the relocated class-A `.desktop.tsx` overrides render (e.g. glass `LeftSidebar`, `PhoneAuthPage` via `AuthGuard`) AND the converted class-B seams render their desktop variants (e.g. desktop hardware-monitor button testid present, desktop Drawer header). End-to-end proof the resolver + registry work in the real desktop bundle.
- **TEST-10** (tier: e2e) [covers: ITEM-2, ITEM-8] file: `src-app/ui/tests/e2e/core/override-fallback.spec.ts` — asserts: the web/core build renders every seam's FALLBACK and does NOT bundle any `.desktop.tsx` (positive control: zero desktop leakage, "no override" is a real tested state).
- **TEST-11** (tier: e2e) [covers: ITEM-11] file: `src-app/desktop/ui/tests/e2e/desktop/gallery-override-runtime.spec.ts` — asserts: the gallery renders the converted seam surfaces + relocated class-A surfaces in fallback (web) and override (desktop) states — including the 390px narrow-viewport state for Drawer/SettingsPage/sidebar — with zero console errors / uncaught exceptions (the `gate:ui` runtime-health contract for the new states).

## Coverage check

- ITEM-1 → TEST-1
- ITEM-2 → TEST-2, TEST-10
- ITEM-3 → TEST-2
- ITEM-4 → TEST-3
- ITEM-5 → TEST-4
- ITEM-6 → TEST-9
- ITEM-7 → TEST-5
- ITEM-8 → TEST-8, TEST-9, TEST-10
- ITEM-9 → TEST-6
- ITEM-10 → TEST-7
- ITEM-11 → TEST-11
- ITEM-12 → TEST-7

Every ITEM covered. `tier: e2e` present (TEST-9/10/11) → UI-diff gate satisfied.
