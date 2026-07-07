# FIX_ROUND-1 — fix the phase-6 ledger, then re-audit

## Fixed (all 12 confirmed phase-6 ledger findings)

1. Selection now cleared on visible-column change too — `use-table-view` effect
   deps `[viewData, visibleColumns]` (was `[sort, query, viewLen]`). Fixes the
   orphaned hidden-column cell selection + the equal-length dataSource-swap case.
2. Keyboard copy path: selectable cells now get `tabIndex=0` (both paths) so a
   click focuses the cell and Ctrl/Cmd+C bubbles to the wrapper handler.
3. Resize handle gains `aria-valuenow`/`aria-valuemin`.
4. `colLabel()` (string title or key) used for resize + column-chooser aria-labels
   → no more `[object Object]`.
5. `onViewChange`/`onSelectionChange` effects now hold the callback in a ref and
   depend only on the view/selection data → no per-render churn.
6. `colMeta` precomputed once per visible column into a `metas` map (both paths).
7. Column-chooser checkbox `disabled` for the last visible column.
8. CSV/formula-injection neutralization added to `rowsToDelimited`
   (`neutralizeFormula`) — prefixes `'` to `=,+,-,@`-leading non-number cells.
9. Row-count readout pluralizes ("1 row" vs "N rows").
10. `selectRow` no longer mutates `anchorRef` inside the state updater
    (StrictMode-safe).
11–12. api-contract / patterns / perms / concurrency review entries — no code
    change required (verified clear).

## Re-audit (fresh blind agent, full diff, all angles)

Verified the 9 code fixes are correctly in place; surfaced **3 new** confirmed
defects:

- **A** (correctness) — Export/Copy still included column-chooser-hidden columns:
  the viewer serialized with the full static `exportColumns`, and the kit never
  surfaced the visible set.
- **B** (security) — the clipboard copy channel (kit Ctrl/Cmd+C + the viewer Copy
  button's `selectionRef`) was NOT formula-neutralized; only the file-export path
  was.
- **C** (state-management/UX) — `filterable` on the SERVER-paginated
  `McpToolCallsTab` filters only the current page while pagination still
  advertises more pages (misleading).

**New confirmed findings:** 3
