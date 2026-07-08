# PLAN_AUDIT — tabular-viewer-ci

Plan audited against the codebase before/while implementing.

## Breakage risk

- `TabularToolbar` props made optional: `XlsxBody` still passes `onCopy`/`onExport`/
  `exportLabel` (now ignored) → compiles unchanged; `DelimitedTable` omits them → OK.
- `DelimitedTable` gains an OPTIONAL `fileId` prop → existing bare call sites
  (`DelimitedViewerDemo`, inline chat via `body.tsx` when `file` is undefined) keep
  working; `publishView()` is a no-op without a `fileId`.
- New `FileStore.fileTabularView` slice is additive; `onFileSync`/`onReconnect` gain
  one more per-file eviction (same pattern as the sibling maps) — no behavior change
  for other slices.
- `DelimitedHeader` gains two buttons; only rendered in the right-panel `{file}`
  context (guarded by the existing `'file' in props`), so inline chat is unaffected.
- Removing `DelimitedTable.onCopy`/`onExport` drops the last consumers of
  `downloadDelimited`/`exportFilename`/`rowsToDelimited` inside that file — moved to
  `tableView.ts` helpers + the header; no other caller of those imports in the file.

## Pattern conformance

- Store slice mirrors `fileWordWrap` (interface + `state` + `Pick<>` + immutable-copy
  setter + sync/reconnect eviction) and the `ImageViewState` type-only import.
- Header buttons mirror `chrome.tsx` `CopyButton`/`DownloadButton` (ghost/icon,
  reactive render read + `$` handler read, `message` toasts, default-top tooltip →
  passes `lint:tooltip-placement`; conventional `Download`/`ClipboardCopy` glyphs →
  passes `lint:icon-action`).
- Gallery surface mirrors the existing `seeded-delimited-viewer` entry + demo.
- Test mirrors the existing `openSeeded` helper and the passing clipboard/download
  patterns already in `mermaid-toggle`/`kit-table-capabilities`.

## Migration collisions

- None. This is a frontend-only change — no SQL migrations, no `migrations/` files
  touched, no DB schema.

## OpenAPI regen

- None. No backend types changed → no `openapi.json` / `api-client/types.ts` regen.
  The only generated frontend artifacts affected are `testIds.generated.ts` and the
  gallery state-matrix (regenerated via `npm run gen:*`), not the OpenAPI client.

## Per-item verdicts

- **ITEM-1** — verdict: PASS — pure helpers reuse existing serializers; DOM helpers sit beside the existing `downloadBlob`.
- **ITEM-2** — verdict: PASS — mirrors the `fileWordWrap` slice idiom exactly; additive.
- **ITEM-3** — verdict: PASS — `fileId` optional; `publishView` no-ops without it; refs already exist.
- **ITEM-4** — verdict: PASS — props already unused; making them optional keeps `XlsxBody` compiling.
- **ITEM-5** — verdict: PASS — mirrors `chrome.tsx` button conventions; distinct testids; disabled-until-published.
- **ITEM-6** — verdict: PASS — one-line prop pass-through; `file?.id` already available in `DelimitedBody`.
- **ITEM-7** — verdict: PASS — mirrors the existing seeded surface; store-free (renders from `text`), avoids the `/text` mock gap.
- **ITEM-8** — verdict: PASS — retarget only surface + button testid; all assertions preserved.
