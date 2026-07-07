# PLAN — kit-table-actions (F1 foundation)

Upgrade the kit `<Table>` with sort / filter / resize / column-chooser / numeric
type-detection / cell-overflow / cell+row selection, then wire it into the tabular
file viewer (CSV/TSV/XLSX) and two data grids (MCP tool-calls, memory audit log).

All work is **pure frontend** (`src-app/ui/**`), client-side. No backend change,
no OpenAPI regen, no migration. Desktop consumes the same kit via the
`@/* → ../../ui/src/*` alias, so there is a single source file per component.

## Items

### Kit `<Table>` capabilities (`components/ui/kit/`)

- **ITEM-1**: Extend `TableColumn<T>` + `TableProps<T>` with new **optional,
  backward-compatible** props. Column: `sortable?`, `sorter?` (custom comparator),
  `numeric?`, `ellipsis?`, `hideable?`, `defaultHidden?`, `resizable?`,
  `minWidth?`, `rowHeader?`. Table: `sortable?`, `filterable?`, `resizable?`,
  `columnChooser?`, `detectNumericColumns?`, `selectionMode?` (`'none'|'cell'|'row'`),
  `toolbar?`, `filterPlaceholder?`, `onCopy?`, `onViewChange?`, `scrollToIndex?`,
  `defaultSort?`. Existing callers (5 usages) keep compiling unchanged.
- **ITEM-2**: View state + derivation. `table-view-core.ts` — a **pure,
  React-free, erasable-TS** module (`compareValues`, `applySort`, `applyFilter`,
  `deriveView`, `detectNumericColumns`, `serializeTsv`, `serializeSelectionTsv`),
  unit-testable via `node --test`. `use-table-view.ts` — the React hook wrapping
  the core and owning widths map, hidden set, and selection state; derives
  `viewData` (filter → sort), `visibleColumns`, per-column numeric flags.
- **ITEM-3**: **Sort** — a sortable header is a `<button>` cycling
  none→asc→desc→none, with `aria-sort` on the `<th>` and a direction glyph.
  Default comparator is numeric-aware (numbers compared numerically, else
  locale string compare); `sorter` overrides. Applies in BOTH plain and
  virtualized render paths.
- **ITEM-4**: **Filter / search** — when `filterable`, the Table renders a
  toolbar search input; rows are kept when any visible column's cell text
  contains the query (case-insensitive substring) or a `filterPredicate`
  matches. A non-empty query with zero matches shows the empty slot.
- **ITEM-5**: **Column resize** — when `resizable`, each resizable header shows a
  drag handle that updates the column's width in the `widths` map (clamped to
  `minWidth`, default 64px). Plain path renders a `<colgroup>` + `table-fixed`;
  virtual path updates the flex `width`. Double-click handle = auto-fit reset to
  the column's declared width.
- **ITEM-6**: **Column-chooser** — when `columnChooser`, the toolbar shows a
  dropdown of hideable columns (checkbox per column) toggling membership in the
  `hidden` set; `defaultHidden` seeds it; the last visible column cannot be
  hidden (guard).
- **ITEM-7**: **Numeric right-align + tabular-nums + type detection** — a numeric
  column (explicit `numeric` or auto-detected) renders header + cells
  right-aligned with `font-variant-numeric: tabular-nums`. Auto-detection samples
  up to 50 non-empty cell values per column and treats a column as numeric when
  ALL sampled values parse as finite numbers.
- **ITEM-8**: **Cell overflow** — an `ellipsis` column truncates its cell to one
  line (`truncate`) and sets the native `title` attribute to the full text when
  the value is a string (hover tooltip). Applies in both paths.
- **ITEM-9**: **Cell / row selection + copy** — when `selectionMode !== 'none'`,
  clicking a data cell selects that cell (`'cell'`) ; clicking a `rowHeader`
  column cell selects the whole row; shift-click extends a row range; cmd/ctrl-click
  toggles a row into the selection. Cmd/Ctrl+C (when the table is focused)
  serialises the selection to TSV, writes it to the clipboard, and calls `onCopy`.
  Selection reads from `viewData` (so it survives row virtualization). Selected
  mounted cells get a ring + `aria-selected`.
- **ITEM-10**: **View escape hatch** — `onViewChange(viewData)` fires from an
  effect whenever the derived view changes (for external readout/export);
  `scrollToIndex` (a view-relative index or null) scrolls the virtualizer
  (`scrollToIndex`) or, in the plain path, `scrollIntoView`s the row. Enables
  jump-to-row + "row X of Y" + export in the viewer without duplicating sort/filter.

### Tabular file viewer (`modules/file/viewers/tabular/`)

- **ITEM-11**: `DelimitedTable` — enable `sortable filterable resizable
  columnChooser detectNumericColumns selectionMode="cell"` on the kit Table; mark
  data columns `ellipsis hideable`; keep the `#` gutter as a `rowHeader` numeric
  non-hideable column. Truncation banner unchanged.
- **ITEM-12**: `XlsxBody` — the same wiring per sheet (mirrors DelimitedTable).
- **ITEM-13**: **Export filtered/sorted view** — a viewer toolbar "Export view"
  action that serialises the current `onViewChange` rows (respecting sort+filter+
  column visibility) to CSV (delimited viewers) / XLSX (xlsx viewer) and triggers
  a client-side download. Distinct from the existing download-original button.
- **ITEM-14**: **Jump-to-row + "row X of Y"** — a viewer readout `Showing {view}
  of {total} rows` plus a number input that maps an entered original row number to
  its current view index and drives the Table's `scrollToIndex`.
- **ITEM-15**: **Copy cell / row / selection (wired)** — a viewer toolbar "Copy"
  button copies the current kit-Table selection (or the whole view when nothing is
  selected) as TSV; relies on ITEM-9. Confirms via a toast.
- **ITEM-16**: **Cell-overflow expand** — clicking a clipped (`ellipsis`) cell in
  the tabular viewer opens a popover showing the full cell value (kit `title`
  gives hover; this adds an explicit click-to-expand affordance).

### Data grids

- **ITEM-17**: `McpToolCallsTab` — enable `sortable filterable` on the kit Table;
  make `Duration` a numeric column; filter/sort operate client-side over the
  currently-loaded page (server pagination unchanged — documented scope).
- **ITEM-18**: `AuditLogSection` — enable `sortable filterable` on the kit Table
  (full client-side over the ≤`limit` loaded rows).

### Housekeeping

- **ITEM-19**: Gallery + generated registries — add gallery story cases/states for
  every new Table capability (sortable, filterable+empty-filtered, resizable,
  column-chooser, numeric, selection) so `check:state-matrix` /
  `check:gallery-coverage` pass; regenerate `KIT_MANIFEST.md` +
  `testIds.generated.ts` (`npm run gen:kit-manifest` + `gen:testid-registry`).

## Files to touch

- `src-app/ui/src/components/ui/kit/table.tsx` — ITEM-1,3,4,5,6,7,8,9,10 (render)
- `src-app/ui/src/components/ui/kit/table-view-core.ts` — **new** — ITEM-2 (pure, React-free derivation/serialisation)
- `src-app/ui/src/components/ui/kit/use-table-view.ts` — **new** — ITEM-2 (React hook wrapping the core)
- `src-app/ui/src/components/ui/index.ts` — re-export any new public types
- `src-app/ui/src/modules/file/viewers/tabular/DelimitedTable.tsx` — ITEM-11,13,14,15,16
- `src-app/ui/src/modules/file/viewers/tabular/XlsxBody.tsx` — ITEM-12,13,14,15,16
- `src-app/ui/src/modules/file/viewers/tabular/tableView.ts` — **new** — shared
  viewer helpers (view→CSV/TSV, view→XLSX, numeric-cell copy) for ITEM-13/15
- `src-app/ui/src/modules/file/viewers/tabular/ExpandableCell.tsx` — **new** — ITEM-16 (truncate + title + click-expand popover)
- `src-app/ui/src/modules/file/viewers/tabular/TabularToolbar.tsx` — **new** — ITEM-13/14/15 (readout + jump + copy + export toolbar)
- `src-app/ui/src/modules/file/viewers/tabular/body.tsx` — ITEM-11 (thread `fileName` through)
- `src-app/ui/src/modules/mcp/components/common/McpToolCallsTab.tsx` — ITEM-17
- `src-app/ui/src/modules/memory/components/sections/AuditLogSection.tsx` — ITEM-18
- `src-app/ui/src/dev/gallery/stories/data.story.tsx` — ITEM-19 (Table + tabular-viewer story cases; e2e surfaces)
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx` — ITEM-19 (loaded MCP tool-calls + memory-audit seeded surfaces for grid e2e)
- `src-app/ui/src/dev/gallery/coverage.ts` + `overlay-allowlist.json` — ITEM-19 (coverage entries for the 2 new components + the expand popover)
- `src-app/ui/src/components/ui/KIT_MANIFEST.md` — ITEM-19 (regenerated)
- `src-app/ui/src/components/ui/testIds.generated.ts` — ITEM-19 (regenerated)
- `src-app/ui/src/dev/gallery/stateMatrix.generated.ts` +
  `galleryCoverage.generated.ts` — ITEM-19 (regenerated if states added)
- **Tests** (Phase 3 enumerates): `use-table-view` unit spec, tabular helper unit
  spec, and e2e specs under `src-app/ui/tests/e2e/`.

## Patterns to follow

- **Kit component idioms** — mirror the EXISTING `table.tsx` (surface hook,
  `data-testid` forwarding + `${testid}-row-${key}`, `alignCls`/`justifyFor`
  maps, virtualization via `useVirtualizer`). New props are optional; the two
  render paths (`PlainTable`, `VirtualTable`) stay the shape they are, both
  consuming the shared `use-table-view` hook. Toolbar controls reuse kit
  primitives: `Input` (search), `Dropdown`/`Popover` + `Checkbox` (column
  chooser), `Button`/`Tooltip` (`components/ui/kit/`). Controllable-state helper
  precedent: `use-controllable-state.ts`, `value-binding.ts`.
- **Viewer chrome** — mirror `viewers/shared/chrome.tsx` (`CopyButton`,
  `DownloadButton`): `Button variant="ghost" size="icon"` with `tooltip`, reading
  `Stores.File.__state` in handlers (never the render proxy —
  [[feedback_stores_state_in_handlers]]). New viewer actions compose into the
  existing `Space`-based `DelimitedHeader`/`XlsxHeader`.
- **Data grids** — keep `McpToolCallsTab` / `AuditLogSection` structure; only add
  the kit Table flags + a numeric column. No new store fields (client-side view).
- **Gallery stories** — mirror the existing `tableStory` shape in
  `data.story.tsx` (`GalleryStory` with `cases[]`), one case per new capability.
- **XLSX export** — reuse the already-present `xlsx` dep (dynamic `import('xlsx')`,
  same as `XlsxBody`); CSV export reuses the delimited round-trip.
- **Selection/copy** — clipboard via `navigator.clipboard.writeText` inside an
  event handler with a `message.success/error` toast, exactly like
  `chrome.tsx::CopyButton`.
