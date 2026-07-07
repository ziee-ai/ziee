# DRIFT-1 — implementation vs plan

Reconciliation of the implemented diff against PLAN.md / TESTS.md / DECISIONS.md
after the first implementation pass. Each divergence is resolved.

- **DRIFT-1.1** — verdict: impl-wins — e2e specs must live under
  `tests/e2e/visual/` (that config boots the gallery Vite webServer; the default
  `tests/e2e/**` config boots the full app + DB). The plan implied
  `tests/e2e/kit-table/`. Amended TESTS.md to point all three specs at
  `tests/e2e/visual/*.spec.ts`; re-ran `--phase 3` (green).

- **DRIFT-1.2** — verdict: impl-wins — added a Table prop `onSelectionChange`
  (not in the original ITEM-1 enumeration) so the viewer's external "Copy"
  button can read the current kit-Table selection (ITEM-15). Additive/optional;
  falls under ITEM-1's "extend TableProps". Documented in DECISIONS DEC-1/DEC-8.

- **DRIFT-1.3** — verdict: impl-wins — the viewer wiring needed two new files
  beyond `tableView.ts`: `ExpandableCell.tsx` (ITEM-16 truncate+title+expand
  popover) and `TabularToolbar.tsx` (ITEM-13/14/15 readout+jump+copy+export).
  Amended PLAN.md "Files to touch"; re-ran `--phase 1` (green).

- **DRIFT-1.4** — verdict: impl-wins — used `data-selected` (not `aria-selected`)
  for the selected-cell/row marker. `aria-selected` on a `<td>` (implicit role
  `cell`) is an axe `aria-allowed-attr` violation and would fail the UI Build
  Gate. Amended TEST-19's assertion wording; re-ran `--phase 3` (green).

- **DRIFT-1.5** — verdict: impl-wins — to drive the tabular viewer + grids from
  the backend-free gallery, exported `XlsxSheet` (renderable from a plain `sheet`
  prop, no binary), added kit + viewer gallery story cases in `data.story.tsx`,
  and added two `holdPatch` "loaded" seeded surfaces (MCP tool-calls, memory
  audit) in `seededSurfaces.tsx`, plus `coverage.ts` + `overlay-allowlist.json`
  entries. Amended PLAN.md "Files to touch" accordingly; all generated gallery
  artifacts regenerated and `npm run check` is green.

- **DRIFT-1.6** — verdict: none — the two data grids implement client-side
  sort/filter over already-loaded data exactly per DEC-5 (MCP tool-calls: current
  server page; memory audit: full ≤`limit` set). No backend/query-param change,
  matching PLAN + PLAN_AUDIT (OpenAPI regen: not required).

- **DRIFT-1.7** — verdict: resolved — numeric right-align is routed through the
  UNCHANGED `alignCls`/`justifyFor` maps (DEC-6); the `lint:logical-direction`
  diff-based gate passes (no new `text-right`/`pl-`/`pr-` literal on any changed
  line). Confirmed green.

**Unresolved drifts:** 0
