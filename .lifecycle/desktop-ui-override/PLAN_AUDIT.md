# PLAN_AUDIT.md — plan audited against the codebase

## Breakage risk

- The core registry + seam primitive are **purely additive** new files; nothing
  imports them until a seam is declared. Zero risk to existing callers.
- The seam-declaring edits (ITEM-5/6) wrap an existing element as
  `useOverride(key, Fallback)` where `Fallback` is the **unchanged** original
  markup. With no registration (the web bundle registers nothing) `resolveOverride`
  returns `undefined` → the Fallback renders → **web behavior is byte-identical**.
  The only runtime cost is one Map `.get` at render, negligible (same cost the
  chat panel registry already pays).
- ITEM-5 DELETES the desktop `HardwareMonitorButton.tsx` shadow. Risk: the desktop
  behavior must be preserved by the registered override. Mitigated by TEST-4
  (unit: override resolves) + TEST-5 (e2e: desktop variant renders + behaves).
- Desktop registration must run BEFORE the core component first renders. Verified
  seam: `main.tsx` calls `loadDesktopModules()` (which `initialize()`s desktop
  modules) before `ReactDOM.render`. If a seam is ever read during a desktop
  module's OWN module-eval (before init), it would miss the registration — audited
  in Phase 6 (state-management angle); the exemplars render inside routed pages,
  well after boot, so safe.

## Pattern conformance

- Registry mirrors `panelRendererRegistry` (`Chat.store.ts:104-150`) — same
  module-level Map, same type-erased-storage / precise-public-edge split, same
  dev-warning-on-miss ergonomics. High conformance.
- Typed keys mirror `Slots` / `PanelRendererMap` declaration merging. High
  conformance.
- Manifest generator mirrors `gen-testid-registry.mjs` (scan both trees → core
  generated file → `--check` in both workspaces). High conformance.
- Desktop registration mirrors the existing desktop-module `initialize()` +
  `setMultiUserMode(false)` pre-render-mutation precedent. High conformance.

## Migration collisions

None — no migration in this feature (BASE.md). No collision possible.

## OpenAPI regen

Not required — no backend/type change. `api-client/types.ts` untouched in both
trees; the phase-3/8 frontend gates still apply because `src-app/ui/**` and
`src-app/desktop/ui/**` (non-generated) files ARE touched.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new file; mirrors `panelRendererRegistry`; no caller broken.
- **ITEM-2** — verdict: PASS — new hook/component; fallback path preserves web behavior exactly.
- **ITEM-3** — verdict: PASS — declaration merging is the established `Slots`/`PanelRendererMap` idiom; type-only, no runtime effect.
- **ITEM-4** — verdict: PASS — registration runs in the existing pre-render desktop-init window; precedent is `setMultiUserMode(false)`.
- **ITEM-5** — verdict: CONCERN — deletes a working desktop shadow; behavior parity must be proven by TEST-4 + TEST-5 before the delete is trusted. Not blocking — the tests are enumerated.
- **ITEM-6** — verdict: CONCERN — the specific host component is chosen at implement time; must pick one whose one-element divergence is genuinely representative (not another trivially-small whole-component case). Resolve by DEC-10 (criteria) during Phase 5; not blocking the plan.
- **ITEM-7** — verdict: PASS — mirrors `gen-testid-registry.mjs`; `--check` slot in `npm run check` is the established gate pattern.
- **ITEM-8** — verdict: PASS — gallery coverage is the standard `check:state-matrix` obligation for a new render state; both galleries already exist.
- **ITEM-9** — verdict: PASS — docs; the generated manifest is the living index.

No `BLOCKED` verdicts. The two `CONCERN`s are covered by enumerated tests /
a Phase-4 decision and do not require a plan amendment.
