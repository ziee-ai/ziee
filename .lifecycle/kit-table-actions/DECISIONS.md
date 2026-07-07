# DECISIONS — kit-table-actions

Every design input resolved up front so implementation runs nonstop. All resolved
by existing convention / codebase; none needed a product call.

### DEC-1: Controlled vs uncontrolled Table view state?
**Resolution:** Uncontrolled — the kit Table owns sort/filter/width/hidden/selection
state internally (via `use-table-view`). Read/drive escape hatches only:
`onViewChange(viewData)`, `onCopy(text)`, `scrollToIndex`, `defaultSort`.
**Basis:** convention — the codebase migrated off antd Table (which was
uncontrolled); the audit explicitly recommends a reusable one-flag primitive so
~6 grids stop re-implementing fragments. Uncontrolled keeps per-grid code to a
single flag.

### DEC-2: Where does the search + column-chooser toolbar render?
**Resolution:** The Table renders its own compact toolbar row above the grid when
`filterable` and/or `columnChooser` are set; hidden otherwise.
**Basis:** convention — self-contained primitive; consumers stay one-flag and the
toolbar sits with the data it controls.

### DEC-3: Scope of the Ctrl/Cmd+C copy handler?
**Resolution:** The copy keydown fires only when focus is inside the Table root
(handler on the root container; guard on `event.currentTarget.contains(document.activeElement)`).
It never hijacks page-wide copy, and does nothing when the selection is empty.
**Basis:** convention — a11y/UX; matches native grid behavior.

### DEC-4: `scrollToIndex` — original-row or view-relative index?
**Resolution:** View-relative (index into the current filtered/sorted `viewData`).
The tabular viewer maps a user-entered ORIGINAL row number to its current view
index using the `onViewChange` rows (each row carries its `__rn`), then sets
`scrollToIndex`.
**Basis:** codebase — TanStack `virtualizer.scrollToIndex` operates on the
rendered list, which is the filtered/sorted view.

### DEC-5: Grid sort/filter — client-side or server-side?
**Resolution:** Client-side over already-loaded data. MCP tool-calls: the current
server page (pagination unchanged). Memory audit: the full ≤`limit` loaded set.
No new query params, no backend change.
**Basis:** convention/scope — F1 is a frontend kit upgrade; the grids currently
have NO sort/filter, so client-side is a strict improvement. Server-side filtering
is a documented follow-up, not a regression.

### DEC-6: Numeric right-align vs the diff-based logical-direction lint?
**Resolution:** Route numeric alignment through the EXISTING, unchanged
`alignCls` (`right→text-right`) and `justifyFor` (`right→justify-end`) maps by
computing an effective `align:'right'` for numeric columns. Introduce no new
`text-right`/`text-left`/`pl-`/`pr-` literal on any changed line.
**Basis:** codebase — `scripts/lint-logical-direction.mjs` scans only added/changed
lines and flags `text-right`; the maps already exist and stay untouched.

### DEC-7: A11y / icon-action naming for the new affordances?
**Resolution:** Sortable header is a real `<button>` whose accessible name is the
column title, with `aria-sort` on the `<th>`. Column-chooser trigger is an icon
`Button` with `tooltip` + `aria-label="Choose columns"`. Resize handle is a
`role="separator"` with `aria-label` + `aria-orientation="vertical"`, keyboard
focusable. Search input has an `aria-label`. All satisfy `lint:icon-action` + WCAG.
**Basis:** codebase — `lint:icon-action`; WCAG 2.1 AA (the UI Build Gate).

### DEC-8: Selection model breadth?
**Resolution:** Single-cell select, whole-row select (via a `rowHeader` column),
shift-click row-range extend, cmd/ctrl-click row toggle. NOT rectangular
multi-cell ranges. Selection is stored as view indices and copy reads from
`viewData`, so it is virtualization-safe (unmounted rows still copy).
**Basis:** convention/scope — covers copy cell/row/selection with a testable,
bounded model.

### DEC-9: Sort/filter vs the tabular viewer's 200-row virtualization threshold?
**Resolution:** Sort/filter operate on the full parsed dataSource (≤`MAX_ROWS`
10k) BEFORE the virtualization decision; the `>200 rows → virtualize` choice is
made on the current view length inside the kit Table.
**Basis:** codebase — DelimitedTable/XlsxBody already virtualize above 200; the
kit Table derives the view first, then picks the path.

### DEC-10: Export format + filename per viewer?
**Resolution:** Delimited viewers export CSV/TSV matching their active delimiter;
the xlsx viewer exports `.xlsx` via dynamic `import('xlsx')` (`aoa_to_sheet` →
`writeFile`/blob). Filename = original file name stem + `-view.{csv|tsv|xlsx}`.
**Basis:** codebase — `XlsxBody` already dynamic-imports `xlsx`; download/blob
pattern follows `FileStore.downloadFile`.

### DEC-11: Cell-overflow — hover title vs click-expand, and click ambiguity with selection?
**Resolution:** Kit provides the hover `title` (ITEM-8). In the tabular viewer, a
truncated ellipsis cell shows an expand icon-button on hover/focus that opens a
kit `Popover` with the full value (ITEM-16); the cell body click still performs
selection. Expand and select therefore never share the same target.
**Basis:** convention — avoids a single click meaning two things.

### DEC-12: Do the grids get a duplicate search box?
**Resolution:** No. The grids keep their existing controls (MCP "Hide built-in";
audit "Show last N") and enable `sortable filterable`; the single search input is
the kit Table's own toolbar. One search surface per grid.
**Basis:** convention — no duplicated affordance.

### DEC-13: Do MarkdownTable / DryRunPreviewDialog adopt the new features?
**Resolution:** No — out of scope. They do not opt in; with all new props optional
and defaulted off, they render byte-identically.
**Basis:** scope — F1 targets the tabular viewer + the two named grids only.

### DEC-14: Sort/aria parity across the plain and virtualized render paths?
**Resolution:** Both `PlainTable` (shadcn `TableHead`) and `VirtualTable` (`<th>`)
render the same sortable-header button + `aria-sort`, resize handle, and numeric
alignment via the shared hook + a shared header-cell helper.
**Basis:** convention — the two paths must behave identically for the same props.

### DEC-15: Where does the viewer toolbar (export / copy / jump / readout) live?
**Resolution:** A slim toolbar row rendered by DelimitedTable/XlsxBody ABOVE the
kit Table (below the truncation banner), NOT in the shared icon-only viewer header
chrome. It holds the "row X of Y" readout, jump-to-row number input, "Copy", and
"Export view" controls.
**Basis:** codebase — the shared header (`chrome.tsx`) is an icon-only `Space`;
labelled inputs/readouts belong in the body-local toolbar.

### DEC-16: Is numeric auto-detection on by default?
**Resolution:** No — `detectNumericColumns` is opt-in (default off). The tabular
viewer opts in; the grids set `numeric` explicitly (MCP Duration) and otherwise
leave columns as-is.
**Basis:** convention — backward-compatibility for the untouched callers.

### DEC-17: Toolbar position relative to the virtualized scroll box?
**Resolution:** The kit toolbar renders OUTSIDE (above) the `ScrollArea`/virtual
scroll box, so it stays fixed while the body scrolls.
**Basis:** convention — matches how a header toolbar behaves over a scroll region.
