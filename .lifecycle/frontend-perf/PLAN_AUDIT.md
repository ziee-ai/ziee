# frontend-perf — PLAN_AUDIT (plan vs codebase)

## Breakage risk

- **ITEM-1** is the one with real breakage surface: the plugin objects
  (`chatMarkdownPlugins`, `STREAMDOWN_PLUGINS`) are consumed by 4 render paths
  (chat TextContent ×2, file markdown viewer, skill/workflow drawers). Moving
  plugin construction behind the lazy boundary must preserve the EXACT plugin set
  per path (chat = base + HtmlBlock renderer; base = code+math+mermaid). Verified
  the only two variants are chat vs base — no third combination exists. The
  `shikiTheme` prop drop is safe (the code plugin ignores it, per source comment).
- **ITEM-2** risk: over-tightening prefetch could delay a route the user WILL
  visit. Mitigated by keeping authenticated+permitted prefetch (just gated), and
  the e2e proves post-login navigation still loads chunks on demand.
- **ITEM-5** is a one-line `shouldMount` fix mirroring siblings — negligible risk.

## Pattern conformance

- ITEM-1 extends the existing `LazyStreamdown` (`lazyWithPreload`→`React.lazy`→
  `Suspense`) rather than inventing a mechanism; preserves the desktop-preload
  override contract by keeping every loader routed through `lazyWithPreload`. ✓
- ITEM-5 mirrors `modules/file/module.tsx` / `modules/scheduler/module.tsx`
  `shouldMount: () => useDelayedFalse(...)`. ✓
- ITEM-2 self-gates on the same permission primitive the route guards use. ✓

## Migration collisions

None — frontend-only feature, zero migrations. No collision with
`202607150000_seed_ledger.sql` (current highest).

## OpenAPI regen

Not required — no backend handler or `#[derive(JsonSchema)]` type changes;
`openapi.json` / `api-client/types.ts` untouched. Therefore the phase-3/8
frontend gates treat this as UI work (correct) but no regen-parity step applies.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — extends LazyStreamdown; 4 call sites, 2 variants, all identified; shikiTheme drop verified safe.
- **ITEM-2** — verdict: PASS — gate on existing permission primitive; touches shared `sdk/packages/shell`, so check desktop loader interaction (DEC-4).
- **ITEM-3** — verdict: CONCERN — eager→lazy module-glob split must preserve the desktop `loader.desktop.ts` name-blocklist semantics; diff the desktop override before shipping. Not blocking; handled at implementation.
- **ITEM-4** — verdict: PASS — removing a competing static import from a barrel is mechanical; each fix verified against the build log's warning list.
- **ITEM-5** — verdict: PASS — one-line sibling-mirrored `shouldMount`.
- **ITEM-6** — verdict: CONCERN — reordering boot (SSE-from-persisted-token / early chunk preload) must not resurrect a session cleared by logout mid-flight; reuse the existing `sessionEpoch` guard. Not blocking; verified at implementation.
- **ITEM-7** — verdict: PASS — dep consolidation; call-site swap + `cargo`/npm check.
- **ITEM-8** — verdict: PASS — fixed-threshold build-time check mirroring existing `check:*` scripts; must read its source-of-truth from a product-tree path (rule B6), not `.lifecycle`.
