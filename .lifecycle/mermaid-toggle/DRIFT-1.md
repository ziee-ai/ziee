# DRIFT-1 — implementation vs PLAN

Reconciling the implemented diff against PLAN.md / DECISIONS.md, item by item.

- **DRIFT-1.1** — verdict: none — ITEM-1 `MermaidBlock.tsx` matches the plan: `components/common`, `[data-streamdown="mermaid-block"]` card mirroring `MarkdownTable.tsx` (same tokens, header bar, ghost-button toolbar, testid style).
- **DRIFT-1.2** — verdict: none — ITEM-2 toggle is the kit `Segmented` (`data-testid="mermaid-source-toggle"`, Diagram/Source), default `render`; body switches diagram/source as planned (DEC-1/3).
- **DRIFT-1.3** — verdict: none — ITEM-3 renders via lazy `import('mermaid')`, `securityLevel:'strict'`, theme from `useThemeOptional().isDark`, defers while `isIncomplete`, catches parse errors into an inline error state, and discards stale async via a cancellation guard — exactly DEC-4/6.
- **DRIFT-1.4** — verdict: none — ITEM-4 copy-source ghost button uses the `MessageActions` clipboard+`message` idiom.
- **DRIFT-1.5** — verdict: none — ITEM-5 download-svg ghost button uses the export-extension Blob/object-URL idiom, disabled until a successful render (DEC-9).
- **DRIFT-1.6** — verdict: none — ITEM-6 shared `mermaidRenderers.ts` is wired via `plugins={mermaidRenderers}` on BOTH `TextContent` render paths; `PluginConfig`/`CustomRenderer` typecheck clean in both ui + desktop.
- **DRIFT-1.7** — verdict: none — ITEM-7 `mermaid.story.tsx` exercises render / source / error / streaming and is registered in `stories/index.ts`.
- **DRIFT-1.8** — verdict: none — ITEM-8 `mermaid-toggle` removed from `affordance-audit-allowlist.json` (now `"allowed": []`).
- **DRIFT-1.9** — verdict: resolved — PLAN's "Files to touch" did not enumerate the MECHANICALLY-GENERATED registry artifacts that adding testids + an import line necessarily regenerate (`components/ui/testIds.generated.ts`, `dev/gallery/stateMatrix.generated.ts`, `dev/gallery/STATE_MATRIX.md`). These were regenerated via `npm run gen:testid-registry` + `npm run gen:state-matrix` (the changes are the 3 new testid literals + line-number shifts in the two TextContent surfaces — no new required state, so `stateCoverage.ts` needed no edit). `npm run check` (ui) is green. Not a behavioral divergence; the plan's intent (no OpenAPI/types regen) is unaffected — these are UI-gate registries the check step mandates.

## Verification performed this round

- `tsc --noEmit` (ui): exit 0.
- `tsc --noEmit` (desktop/ui, which compiles the shared component via `@/*`→`../../ui/src`): exit 0.
- `npm run check` (ui): exit 0 — tsc + biome guardrails + lint:colors + lint:settings-field + lint:adjacent-inline + lint:icon-action + check:kit-manifest + check:testid-registry + check:design-spec + check:gallery-coverage + check:gallery-crawl + gallery:check-fixtures + check:state-matrix all pass.

**Unresolved drifts:** 0
