# PLAN_AUDIT — kit-table-actions

Audit of PLAN.md against the actual codebase (worktree off origin/main).

## Breakage risk

The kit `<Table>` has **7 non-gallery callers** + **3 gallery stories**:
`MarkdownTable.tsx`, `DelimitedTable.tsx`, `XlsxBody.tsx`, `McpToolCallsTab.tsx`,
`AuditLogSection.tsx`, `DryRunPreviewDialog.tsx`, and the `data`/`composite`/
`stress` stories. **Every new prop in ITEM-1 is optional** and every new behavior
is gated behind an opt-in flag (`sortable`/`filterable`/`resizable`/
`columnChooser`/`selectionMode`/`detectNumericColumns`), so all 10 callers keep
compiling and rendering **byte-identically** when they don't opt in. The default
`selectionMode` is `'none'`, default sort/filter/resize/chooser are off, so
`MarkdownTable` + `DryRunPreviewDialog` (which we do NOT wire) are untouched at
runtime. Verified: `tsc` is the guard — a required-prop addition would break all
callers; we add none.

The two render paths (`PlainTable`, `VirtualTable`) are refactored to consume a
shared `use-table-view` hook, but their **public output shape** (root `data-testid`,
`${testid}-row-${key}` rows, sticky header, virtualization) is preserved — the
existing e2e selectors (`file-delimited-table`, `mcp-tool-calls-table`,
`memory-audit-table`, `g-table`) stay valid. Risk: the plain path gains a
`<colgroup>` + `table-fixed` **only when `resizable`** — non-resizable callers
keep auto-layout. Low risk; guarded by the flag.

## Pattern conformance

- **ITEM-1..10** mirror the existing `table.tsx` idioms (surface hook, testid
  forwarding, `alignCls`/`justifyFor` maps, `useVirtualizer`). Column-chooser uses
  the kit `Popover` (`content` + trigger child) + `Checkbox`; search uses kit
  `Input`; toolbar buttons use kit `Button`+`Tooltip`. All confirmed exported from
  `components/ui/index.ts`. Controllable-state precedent exists
  (`use-controllable-state.ts`).
- **Logical-direction lint is diff-based and bans `text-right`/`text-left`/`pl-`/
  `pr-`/`ml-`/`mr-` on changed lines.** The existing `alignCls`(`right→text-right`)
  and `justifyFor`(`right→justify-end`) maps stay **unchanged**; numeric columns
  compute an effective `align:'right'` routed through those unchanged maps, so no
  new physical-direction literal enters the diff. `justify-end`/`text-end` are the
  logical forms already in use. (DEC-6.)
- **ITEM-11..16** mirror `viewers/shared/chrome.tsx` (ghost icon `Button` +
  `tooltip`, `Stores.File.__state` in handlers per
  [[feedback_stores_state_in_handlers]], `message.success/error` toast). New
  actions compose into the existing `Space`-based headers.
- **ITEM-17/18** keep the grid components' structure; add only kit flags + a
  numeric column. No store changes.
- **ITEM-19** mirrors the existing `tableStory` in `data.story.tsx`.
- **Icon-only affordances** (sort glyph is on a text header button; column-chooser
  trigger, resize handle) get explicit `aria-label`s to satisfy `lint:icon-action`
  + a11y (DEC-7).

## Migration collisions

None. This feature is **pure frontend, client-side**. `ls migrations/` is
irrelevant — no SQL migration is added. No new DB tables, columns, or permissions.

## OpenAPI regen

**Not required.** No Rust type, handler, or response schema changes; no
`openapi.json` / `api-client/types.ts` delta. Both grids sort/filter data the
client already fetched via existing endpoints (`GET /api/mcp/tool-calls`,
memory-audit load) — no new query params. Confirmed: the diff will touch only
`src-app/ui/**` (kit + module + gallery + generated `KIT_MANIFEST.md`/
`testIds.generated.ts`, which are the FRONTEND generators, not the openapi ones).

## Per-item verdicts

- **ITEM-1** — verdict: PASS — all new props optional; 10 callers keep compiling (tsc-guarded)
- **ITEM-2** — verdict: PASS — new pure hook file; precedent `use-controllable-state.ts`
- **ITEM-3** — verdict: PASS — sort applies in both paths via the shared hook; aria-sort standard
- **ITEM-4** — verdict: PASS — toolbar search reuses kit `Input`; empty-result → existing empty slot
- **ITEM-5** — verdict: CONCERN — resize adds `<colgroup>`+`table-fixed` to the plain path ONLY under `resizable`; must verify non-resizable callers keep auto-layout (guarded; covered by TEST for both paths)
- **ITEM-6** — verdict: PASS — column-chooser via kit `Popover`+`Checkbox`; last-visible guard
- **ITEM-7** — verdict: CONCERN — numeric right-align must route through the UNCHANGED `alignCls`/`justifyFor` maps to avoid the diff-based logical-direction lint flagging `text-right` (DEC-6)
- **ITEM-8** — verdict: PASS — `truncate` + native `title`; additive per-column
- **ITEM-9** — verdict: CONCERN — selection reads from `viewData` (survives virtualization); cmd/ctrl+C handler must be scoped to a focused table container to avoid stealing global copy; covered by DEC-3 + tests
- **ITEM-10** — verdict: CONCERN — `onViewChange` must fire from an effect (not during render) to avoid setState-in-render; `scrollToIndex` view-relative semantics fixed in DEC-4
- **ITEM-11** — verdict: PASS — mirrors current DelimitedTable; `#` gutter becomes `rowHeader` numeric
- **ITEM-12** — verdict: PASS — mirrors XlsxBody per-sheet
- **ITEM-13** — verdict: PASS — reuses `xlsx` dep (dynamic import) + delimited round-trip; client-side blob download
- **ITEM-14** — verdict: PASS — readout + number input drive `scrollToIndex`; original-row→view-index map from `onViewChange`
- **ITEM-15** — verdict: PASS — Copy button reads kit selection (or whole view); toast like `chrome.tsx`
- **ITEM-16** — verdict: PASS — click-to-expand popover over the kit `title` hover; additive
- **ITEM-17** — verdict: CONCERN — client-side sort/filter scopes to the loaded server page only; documented, not a regression (grid had none). DEC-5
- **ITEM-18** — verdict: PASS — audit log loads ≤`limit` rows in one shot; client-side view is complete
- **ITEM-19** — verdict: CONCERN — new "empty-filtered"/capability states may trip `check:state-matrix`/`check:gallery-coverage`; ITEM-19 budgets the gallery cells + regenerates `KIT_MANIFEST.md`/`testIds.generated.ts` (must run `gen:*` before `check`)

No `BLOCKED` verdicts. The five `CONCERN`s are all resolved in DECISIONS.md
(DEC-3..7) with concrete approaches; none requires a plan amendment.
