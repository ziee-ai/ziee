# PLAN — tabular-viewer-ci: header-driven Export/Copy hookup

Fix the two failing Layer-A visual tests (`tabular-viewer.spec.ts` TEST-23 export,
TEST-25 copy) by completing the file-viewer **header-driven** Export / Copy-selection
hookup. Root cause: commit `643cbc6f` (by pbya, intended) removed the body-toolbar
Copy/Export buttons the tests clicked, orphaning the view-export + selection-copy
features; the seeded surface renders a bare `DelimitedTable`, so the buttons never
appear → 60 s timeouts. We surface those view-aware actions in the header (pbya's
stated "future header-driven hookup"), coordinated body→header through `FileStore`.

## Items

- **ITEM-1**: Add `TabularViewState` snapshot type + `exportTabularView`, `copyTabularSelection`, and the pure `tabularClipboardText` helpers to `tableView.ts` (reusing the existing `rowsToDelimited`/`downloadDelimited`/`exportFilename`).
- **ITEM-2**: Add a `fileTabularView: Map<fileId, TabularViewState>` slice + `setFileTabularView` action to `File.store.ts` (interface, `state`, `Pick<>` union, action, and `onFileSync`/`onReconnect` cleanup), mirroring the `fileWordWrap` idiom.
- **ITEM-3**: `DelimitedTable` accepts a `fileId?` prop and publishes the current view snapshot via `publishView()` on mount + `onViewChange`/`onSelectionChange`; remove the now-orphaned local `onCopy`/`onExport`.
- **ITEM-4**: `TabularToolbar` retires the vestigial `onCopy`/`onExport`/`exportLabel` props (make optional) and updates its docstring (hookup now lives in the header).
- **ITEM-5**: `DelimitedHeader` renders view-aware **Copy selection** (`file-viewer-tabular-copy-btn`) + **Export view** (`file-viewer-tabular-export-btn`) buttons that read `FileStore.fileTabularView` and call the ITEM-1 helpers; disabled until a snapshot exists.
- **ITEM-6**: `body.tsx` threads `fileId={file?.id}` into `DelimitedTable`.
- **ITEM-7**: Add a header-inclusive CSV gallery surface — `DelimitedViewerWithHeaderDemo` in `TableDemos.tsx` + the `seeded-delimited-viewer-shell` entry in `seededSurfaces.tsx` (renders `DelimitedHeader` over the real `DelimitedTable`, no async `/text` load).
- **ITEM-8**: Retarget `tabular-viewer.spec.ts` TEST-23 + TEST-25 to `seeded-delimited-viewer-shell` and the header button testids; all assertions unchanged (coverage preserved).

## Files to touch

- `src-app/ui/src/modules/file/viewers/tabular/tableView.ts`
- `src-app/ui/src/modules/file/viewers/tabular/tableView.test.ts`
- `src-app/ui/src/modules/file/stores/File.store.ts`
- `src-app/ui/src/modules/file/viewers/tabular/DelimitedTable.tsx`
- `src-app/ui/src/modules/file/viewers/tabular/TabularToolbar.tsx`
- `src-app/ui/src/modules/file/viewers/tabular/header.tsx`
- `src-app/ui/src/modules/file/viewers/tabular/body.tsx`
- `src-app/ui/src/dev/gallery/TableDemos.tsx`
- `src-app/ui/src/dev/gallery/seededSurfaces.tsx`
- `src-app/ui/tests/e2e/visual/tabular-viewer.spec.ts`
- `src-app/ui/src/components/ui/testIds.generated.ts` (regenerated: new testids)

## Patterns to follow

- **Store slice** — mirror `File.store.ts`'s `fileWordWrap` exactly: `Map<fileId, T>`
  in the interface + `state` + `Pick<>`; an immutable-Map-copy setter next to
  `setFileWordWrap`; per-file eviction in `onFileSync`/`onReconnect`. Type-only import
  of the viewer type mirrors the existing `ImageViewState` import.
- **Header buttons** — mirror `viewers/shared/chrome.tsx` (`CopyButton`/`DownloadButton`):
  `Button variant="ghost" size="icon" tooltip=… aria-label=… icon={…}`, reactive read
  in render + `Stores.File.$` raw read in the click handler, `message.success/error`.
- **Gallery surface** — mirror the existing `seeded-delimited-viewer` entry in
  `seededSurfaces.tsx` + `DelimitedViewerDemo` in `TableDemos.tsx`.
- **Test** — mirror the existing `tabular-viewer.spec.ts` `openSeeded` helper + the
  passing kit-table/mermaid clipboard/download patterns (`navigator.clipboard`,
  `context.grantPermissions`, `Promise.all([waitForEvent('download'), click])`).
