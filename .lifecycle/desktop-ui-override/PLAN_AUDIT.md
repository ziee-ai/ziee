# PLAN_AUDIT.md — plan audited against the codebase

## Breakage risk

- **Registry + seam primitives (ITEM-1/2/3)** are additive new files; nothing
  imports them until a seam is declared. The seam wrap keeps the original markup as
  the fallback, so with no registration (web) behavior is byte-identical. Runtime
  cost = one `Map.get` per seam at render (the cost the panel registry already
  pays). Low risk.
- **Resolver edit (ITEM-5)** touches the load-bearing `localOverridePlugin`. This
  is the highest-risk change: a regression breaks EVERY `@/` import in the desktop
  build. Mitigation: extract the resolution order into a pure function with unit
  tests (TEST-4) covering all three tiers + the no-match fall-through; the existing
  desktop overrides act as a live regression corpus (the desktop build must still
  boot — TEST-9). MEDIUM risk, mitigated.
- **Web tsconfig/biome exclude (ITEM-5)** — `**/*.desktop.*` must exclude ONLY the
  co-located desktop files, not legitimate core files. No core file uses a
  `.desktop.` infix today (verified: the only `.desktop`-ish names are under test
  dirs). Low risk.
- **`.desktop.tsx` cross-workspace dependency (ITEM-6)** — relocating Tauri-importing
  files into `ui/src` means the web workspace physically contains Tauri code. It is
  never bundled (web vite never imports it) and excluded from web tsc/biome, so it's
  inert there; the desktop workspace typechecks it (its `include` already has
  `../../ui/src` + it has the tauri deps). Accepted tradeoff (DEC-13). Risk: the
  `testidUniquePlugin` two-tree scan could flag a `.desktop.tsx` testid as a
  duplicate of its core sibling — ITEM-5 updates the scanner to treat `.desktop` as
  a shadow. MEDIUM risk, explicitly handled.
- **Codemod (ITEM-7/8)** — AST rewrites of 5 subtly-diverging files. Full
  auto-migration was chosen (DEC-12); the mitigation is that output is
  human-reviewed and fixture-tested (TEST-5), and per-seam parity is asserted
  (TEST-8). The known Drawer drift is NOT propagated (ITEM-9 / TEST-6). MEDIUM risk,
  mitigated by review + tests.
- **Deleting the 5 class-B shadows (ITEM-8)** — desktop behavior must be preserved
  by the registered overrides; asserted by TEST-8 (unit parity) + TEST-9 (e2e). The
  `data-testid`s on desktop variants are preserved so existing desktop e2e keeps
  passing.

## Pattern conformance

- Registry mirrors `panelRendererRegistry` (high). Typed keys mirror
  `Slots`/`PanelRendererMap` (high). Manifest/`--check` mirrors
  `gen-testid-registry.mjs` (high). Resolver edit extends the existing probe loop
  in the same plugin (high). Desktop registration mirrors the existing
  desktop-module init + `setMultiUserMode(false)` precedent (high).
- New surface: the ts-morph codemod has no in-repo precedent (existing `.mjs`
  scripts are string/AST-light generators). Phase 5 must confirm `ts-morph` (or an
  equivalent) is an acceptable devDependency; if not, fall back to the TypeScript
  compiler API directly. Flagged, not blocking.

## Migration collisions

None — no DB migration (BASE.md). No collision possible.

## OpenAPI regen

Not required — no backend/type change. `api-client/types.ts` untouched in both
trees. The frontend phase-3/8 gates still apply (non-generated `ui/**` +
`desktop/ui/**` files touched).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — new file; mirrors `panelRendererRegistry`.
- **ITEM-2** — verdict: PASS — new primitives; fallback path preserves web behavior.
- **ITEM-3** — verdict: PASS — established declaration-merging idiom; type-only.
- **ITEM-4** — verdict: PASS — runs in the existing pre-render desktop-init window.
- **ITEM-5** — verdict: CONCERN — edits the load-bearing resolver + web tsconfig +
  testid scanner; must be pure-function unit-tested (TEST-4) and boot-verified
  (TEST-9). Not blocking — mitigations enumerated.
- **ITEM-6** — verdict: CONCERN — puts Tauri code in the web tree; inert there but
  a novel cross-workspace layout. Accepted per DEC-13; covered by TEST-9/10.
- **ITEM-7** — verdict: CONCERN — ts-morph codemod, no in-repo precedent + subtle
  rewrites. Mitigated by reviewed output + fixture tests (TEST-5); devDep confirmed
  in Phase 5.
- **ITEM-8** — verdict: CONCERN — deletes 5 working shadows; parity must be proven
  (TEST-8 + TEST-9) before the deletes are trusted.
- **ITEM-9** — verdict: PASS — restores dropped core behavior (a fix); asserted by
  TEST-6. Strictly improves desktop correctness.
- **ITEM-13** — verdict: PASS — the infra `.desktop.ts` relocations (loader,
  App.store, lazyWithPreload, getBaseURL) are behavior-preserving tier-1→tier-2
  moves (all `@/`-imported; identical resolution result), verified by both tsc, the
  desktop `vite build`, and `npm run check`. The auth barrel delete is safe (nothing
  imports the `@/modules/auth` barrel form — verified). SidebarHeaderSpacer→seam is
  covered by TEST-8.
- **ITEM-14** — verdict: PASS — the gate is a pure enumeration + DECISIONS parse
  (TEST-12); wired into the existing `npm run check` chain in both workspaces; no
  runtime/product impact (build-time dev tooling, like the other generators).
- **ITEM-10** — verdict: PASS — mirrors `gen-testid-registry.mjs`; `--check` in
  `npm run check` is the established gate.
- **ITEM-11** — verdict: PASS — standard `check:state-matrix` obligation; both
  galleries exist.
- **ITEM-12** — verdict: PASS — docs; the generated manifest is the living index.

No `BLOCKED` verdicts. The five `CONCERN`s are the aggressive-path risks the human
explicitly accepted (DEC-12/DEC-13); each is covered by an enumerated test and a
named mitigation, so none requires a plan amendment.
