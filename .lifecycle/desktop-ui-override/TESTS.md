# TESTS.md — enumerated tests (bipartite: every ITEM ↔ ≥1 TEST)

No new permission is introduced (this is build/render infrastructure), so no
A9 backend-deny or A10 `[negative-perm]` e2e is required. UI paths ARE touched,
so ≥1 `tier: e2e` is enumerated (TEST-5, TEST-6, TEST-8).

- **TEST-1** (tier: unit) [covers: ITEM-1] file: `src-app/ui/src/core/overrides/registry.test.ts` — asserts: `registerOverride` then `resolveOverride` returns the registered component; `resolveOverride` on an unregistered key returns `undefined`; a second register for the same key replaces (last-wins) and is observable.
- **TEST-2** (tier: unit) [covers: ITEM-2, ITEM-3] file: `src-app/ui/src/core/overrides/useOverride.test.tsx` — asserts: `useOverride(key, Fallback)` renders the Fallback when nothing is registered, and renders the override component when one is registered; the `<Override>` wrapper forwards props to whichever component wins. (Type-level `keyof UIOverrides` enforcement is proven by `tsc` in `npm run check`.)
- **TEST-3** (tier: unit) [covers: ITEM-4] file: `src-app/desktop/ui/src/modules/desktop-base/overrides.test.ts` — asserts: invoking the desktop registration entry point registers exactly the expected seam keys (pure — call it, then `resolveOverride(key)` is defined for each), and does so without touching the DOM (safe at module-init time).
- **TEST-4** (tier: unit) [covers: ITEM-5] file: `src-app/desktop/ui/src/modules/hardware/hardware-override.test.tsx` — asserts: with the desktop registration applied, the `hardware.monitor-button` seam resolves to the native-window desktop variant; without it, the seam renders the core fallback (the original browser-popup button). Proves the deleted whole-file shadow is fully replaced.
- **TEST-5** (tier: e2e) [covers: ITEM-5, ITEM-6] file: `src-app/desktop/ui/tests/e2e/desktop/desktop-override-seam.spec.ts` — asserts: booting the DESKTOP build, the converted seam surfaces render their DESKTOP variants (e.g. the desktop hardware-monitor button testid is present / the ITEM-6 element shows its desktop form), proving the registry swaps in the real desktop bundle end-to-end.
- **TEST-6** (tier: e2e) [covers: ITEM-6] file: `src-app/ui/tests/e2e/core/override-fallback.spec.ts` — asserts: the SAME seam, in the web/core build (which registers no overrides), renders the FALLBACK element — the positive control proving the seam is zero-impact for web and that "no override" is a real, tested state.
- **TEST-7** (tier: unit) [covers: ITEM-7, ITEM-9] file: `src-app/ui/scripts/gen-override-registry.test.mjs` — asserts: the generator emits a manifest listing all declared seams; `--check` PASSES when every registered override matches a declared seam key and FAILS (non-zero) when an override references an undeclared key (dead override). The emitted `OVERRIDE_MANIFEST.md` is the doc index for ITEM-9.
- **TEST-8** (tier: e2e) [covers: ITEM-8] file: `src-app/desktop/ui/tests/e2e/desktop/gallery-override-runtime.spec.ts` — asserts: the gallery renders the converted seam surfaces in both their fallback (web gallery) and override (desktop gallery) states with zero console errors / uncaught exceptions (the `gate:ui` runtime-health contract for the new states from ITEM-8).

## Coverage check

- ITEM-1 → TEST-1
- ITEM-2 → TEST-2
- ITEM-3 → TEST-2
- ITEM-4 → TEST-3
- ITEM-5 → TEST-4, TEST-5
- ITEM-6 → TEST-5, TEST-6
- ITEM-7 → TEST-7
- ITEM-8 → TEST-8
- ITEM-9 → TEST-7

Every ITEM covered. `tier: e2e` present (TEST-5, TEST-6, TEST-8) → UI-diff gate satisfied.
