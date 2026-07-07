# DRIFT-1 — implementation vs plan (round 1)

Compared the working tree (`git diff origin/main`) against PLAN.md + DECISIONS.md
after implementing all 8 ITEMs. `tsc` + `npm run check (ui)` are green.

## Divergences

- **DRIFT-1.1** — verdict: impl-wins — PLAN "Files to touch" omitted `src-app/ui/src/dev/gallery/coverage.ts`. The hand-maintained coverage map is a tsc-enforced complete `Record` keyed by every discovered component; the new `HtmlBlock.tsx` component forced a `{ kind: 'via' }` entry there. Not anticipated in the plan. PLAN amended to list it. (Behavior unchanged; purely an omitted touch-point.)
- **DRIFT-1.2** — verdict: impl-wins — PLAN "Files to touch" omitted the regenerated gallery/testid snapshot artifacts (`testIds.generated.ts`, `stateMatrix.generated.ts`, `galleryCoverage.generated.ts`, `STATE_MATRIX.md`). Adding a testid + a gallery fixture block + an interaction recipe changes these `gen-*.mjs --check` snapshots (per DEC-8/DEC-9); they were regenerated via the `gen:` scripts. PLAN amended to list them. (These are mechanically generated and verified by the `--check` gate; unlike `openapi.json`/`types.ts` they are NOT in the validator's coverage-exclude set, so they are enumerated in AUDIT_COVERAGE.tsv at Phase 6.)
- **DRIFT-1.3** — verdict: none — ITEM-1..ITEM-8 otherwise match the plan and every DECISION (DEC-1..DEC-12): default Code view, `sandbox="allow-scripts"` only, `srcDoc` + injected CSP (`default-src 'none'`), `isIncomplete`→force-code, copy+lang header, shared `streamdownPlugins` on both TextContent instances, gallery fixture+`html-preview` interaction, guarding `html-render` detector rule, and the e2e spec (incl. the parent-isolation security proof). No behavioral divergence.

## Reconciliation

Both impl-wins drifts are pure "Files to touch" omissions (no behavior change);
PLAN.md was amended to add the five files and Phases 1–3 re-gated green.

**Unresolved drifts:** 0
