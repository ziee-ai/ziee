# DECISIONS — tabular-viewer-ci

Every human/product input resolved up front.

### DEC-1: Restore the removed body-toolbar buttons, or complete the header hookup?
**Resolution:** Complete the header-driven hookup (surface view-aware Export /
Copy-selection in `DelimitedHeader`); do NOT restore the body-toolbar buttons.
**Basis:** user — the owner confirmed the button removal in `643cbc6f` (authored by
pbya, not the owner) is intended; the commit's own comment names a "future
header-driven hookup" as the direction.

### DEC-2: Coordinate body→header by publishing DATA or registering CALLBACKS in the store?
**Resolution:** Publish a DATA snapshot (`TabularViewState`) into
`FileStore.fileTabularView`; the header runs the export/copy via pure helpers.
**Basis:** convention — the FileStore holds per-file DATA (`fileViewModes`,
`fileTextContents`, `fileWordWrap`), never behavior; a data slice mirrors that and
keeps the serializer path unit-testable.

### DEC-3: What testids for the two header buttons?
**Resolution:** `file-viewer-tabular-copy-btn` and `file-viewer-tabular-export-btn`.
**Basis:** convention — the `file-viewer-*` header-action family; a distinct
`-tabular-*` infix avoids colliding with the existing `file-viewer-copy-btn`
(whole-file copy) / `file-viewer-copy-selection-btn` / `file-viewer-download-btn`.

### DEC-4: New gallery surface, or reuse the bare `seeded-delimited-viewer`?
**Resolution:** Add a new `seeded-delimited-viewer-shell` surface; leave
`seeded-delimited-viewer` bare.
**Basis:** convention — the visual suite uses isolated single-purpose surfaces; a new
surface keeps TEST-21/22/24/26 (which drive the bare table) untouched.

### DEC-5: Render the full `FilePanel` shell (with async `/text`) or `DelimitedHeader`+`DelimitedTable` directly?
**Resolution:** Render `DelimitedHeader` over `DelimitedTable` directly, feeding the
table its `text` prop.
**Basis:** codebase — the gallery mock has no `File.getTextContent` cassette (`/text`
returns empty), so the full shell would render an empty table; the direct composition
exercises the same header→body integration without the async gap.

### DEC-6: Is XLSX (which has the same orphaned export/copy) in scope?
**Resolution:** Out of scope — leave `XlsxBody`/`XlsxHeader` unchanged.
**Basis:** user/codebase — no failing test covers XLSX; its orphan predates this work
(`643cbc6f`, `XlsxHeader` returns `null`); XLSX is multi-sheet and needs per-sheet
keying — a separate follow-up. This change introduces no XLSX regression.

### DEC-7: Copy-selection button label/tooltip wording?
**Resolution:** "Copy selection" (view selection) distinct from the header's whole-file
"Copy"; export tooltip is "Export view".
**Basis:** convention — precise labels neutralize the "confusing duplicate" concern that
motivated the original removal; matches the `chrome.tsx` `CopySelectionButton` naming.
