# DECISIONS — file-viewer-virtualization

Every human/product input resolved up front so implementation runs nonstop. The
three starred decisions (DEC-1, DEC-4, DEC-7) are the ones surfaced to the user
for explicit ack before phase 5.

### DEC-1: TEXT — client-side line-windowing of the full text vs chunked/lazy network loading? ★
**Resolution:** Client-side windowing of the full (already-fetched) text. The
full file content is fetched once (as today, bounded by the 10 MB byte cap), then
split into fixed-size LINE CHUNKS rendered as DOM slots; only the Shiki HIGHLIGHT
of a chunk is deferred until it scrolls into view. No new network/chunk-loading
endpoint. This mirrors `pdf/body.tsx` structurally (all slots mounted +
`content-visibility:auto` + heavy work lazy) — except the PDF's lazy unit is a
server-rasterized image, whereas ours is a purely client-side highlight pass.
**Basis:** codeboase + convention — the text content endpoint already returns the
whole file; the expensive part is Shiki tokenization + the giant highlighted DOM,
not the fetch. Keeping the fetch whole avoids a backend change and preserves
find-in-document (DEC-2). A network-chunked loader would need a new ranged
content endpoint (backend scope) for no benefit under the 10 MB bound.

### DEC-2: TEXT — HOW does Shiki highlight ONLY the visible window without re-highlighting the whole file on scroll? ★ (sub-decision of DEC-1)
**Resolution:** Per-chunk highlight with a cache. The text is split into chunks of
`RAWCODE_CHUNK_LINES = 500` lines. Each chunk is a DOM container with
`content-visibility:auto` + a reserved `contain-intrinsic-size` height, rendered
as PLAIN escaped text (with the same `.line`/`.line-number`/`.line-code` grid
structure Shiki emits, so no layout shift on upgrade) until it intersects the
viewport (+ `rootMargin: '600px 0px'` prefetch, mirroring the PDF observer).
On intersection, that chunk is highlighted ONCE via the existing `codeToHtml` +
`lineNumberTransformer` (the transformer takes a `startLine` offset closure so
global line numbers stay continuous), the resulting HTML is memoized in a
per-instance `Map<chunkIndex, html>`, and swapped in. A chunk already in the
cache is never re-highlighted, so scrolling back and forth is free. Offscreen
chunks are NOT reverted to plain (kept highlighted) — the highlighted-HTML memory
is bounded by the line cap, and reverting would thrash find's MutationObserver.
**Basis:** convention — reuses the existing Shiki path per-chunk rather than
inventing a token-level renderer; the cache + one-way plain→highlight upgrade is
the minimal correct mechanism. Chunk size 500 keeps each highlight pass small
(<~10 ms) while keeping the chunk/observer count modest even for 300k lines.

### DEC-3: TEXT — do offscreen lines stay in the DOM (find requirement)?
**Resolution:** Yes. Every line's text node is always present in the DOM (plain or
highlighted). We do NOT use windowed mounting (react-virtual style) for text.
**Basis:** codebase — `useFindInDocument` walks DOM text nodes via a TreeWalker;
its own comment states it "survives … `content-visibility` virtualization". True
unmounting would break find (only mounted lines would be searchable) and force a
find rewrite. Keeping all text nodes present is the property that lets find span
the whole file with zero changes to the find subsystem (ITEM-3).

### DEC-4: TABULAR — stay CLIENT-side (parse full file + virtualize all rows) or add SERVER-side paging? ★
**Resolution:** Stay client-side. Parse the FULL file into `dataSource` (removing
the `slice(0, MAX_ROWS)` head-cap); the kit `Table` already virtualizes row
rendering (`useVirtualizer`, only visible rows mount) above
`VIRTUALIZE_ROW_THRESHOLD=200`, and sort/filter (`applySort`/`applyFilter` in
`table-view-core`) run over the whole in-memory array — so sort/filter correctly
span the entire file. No server paging, no new endpoint.
**Basis:** codebase + convention — the kit Table's virtualization + full-array
sort/filter already exist; the 10k cap was an arbitrary head-truncation that,
post-F1 (sortable/filterable grid), produces WRONG results (sorting a 10k head of
a 200k file). Server paging would require server-side sort/filter/typed-column
endpoints (large backend scope) and would still give semantically-partial
sort/filter unless fully pushed down — not worth it under the 10 MB bound.
Cost at the bound: a 10 MB CSV is ~50k–200k rows; an in-memory array of that many
record objects + an O(N log N) sort is well within browser budget (<~100 ms
sort, tens of MB), and only visible rows ever hit the DOM.

### DEC-5: TABULAR — how big before client-side hurts, and where is the bound?
**Resolution:** Both CSV/TSV and XLSX keep a RAISED per-viewer OOM-backstop cap
(up from 10k), because the 10 MB byte cap bounds *bytes*, NOT *row count* — a
10 MB CSV of tiny rows (`a\n`) is millions of rows, which would OOM a client
array. So: CSV/TSV → `DELIMITED_MAX_ROWS = 300_000`; XLSX → `XLSX_MAX_ROWS =
200_000` (lower because xlsx is heavier per row: zip-decompress + per-cell object,
and the byte cap can't bound its decompressed count at all). XLSX applies the cap
to BOTH `XLSX.read({ sheetRows: XLSX_MAX_ROWS + 1 })` and the `slice`. For every
REAL (wide-row) file both caps sit far above the true row count, so they never
truncate in practice — they only bite the pathological narrow-row case. The
existing truncation banner fires only at these raised thresholds.
**Basis:** user constraint ("keep a sane upper bound so we never OOM") + codebase.
Correction over the pre-ack summary, which wrongly said CSV is "byte-bounded 1:1"
and needs no cap — bytes don't bound row count, so a raised backstop is retained.
Strictly safer; does not change the acked client-side / no-server-paging decision.

### DEC-6: TEXT — is there still a per-viewer line cap, and at what value?
**Resolution:** Yes, a raised OOM backstop `RAWCODE_MAX_LINES = 300_000` (up from
10k). Below it the full file renders windowed; at/above it the file is truncated
to the cap with the existing banner. This exists only to bound the pathological
"10 MB of newline characters → millions of single-char DOM line rows" case that
the byte cap cannot catch (byte cap allows ~10M newlines). 300k line rows with
`content-visibility:auto` is comfortably renderable; the vast majority of real
files are far below it and never truncate.
**Basis:** convention — analogous to the xlsx cap: keep a cap where the byte cap
doesn't bound the true DOM-cost dimension, but raise it ~30× so it stops being a
truncation-UX and becomes an OOM guard only.

### DEC-7: FilePanel byte-cap backstop — where does it sit after these changes? ★
**Resolution:** Unchanged: keep `PREVIEW_SIZE_LIMIT_BYTES = 10 MB` at FilePanel as
the SINGLE outer OOM backstop that prevents even fetching a pathological file.
The per-viewer caps become OOM guards (DEC-5/DEC-6), not preview-truncation UX. We
do NOT raise the 10 MB cap — the feature's goal was that the arbitrary 10k
per-viewer caps were the wrong bound, not that 10 MB is too small; 10 MB of
text/CSV is already "large" (tens-to-hundreds of thousands of lines/rows) and
raising it increases OOM risk on the memory-heavy paths (xlsx decompress, full
in-memory dataSource, whole-file DOM).
**Basis:** user constraint ("don't remove the safety backstop … keep a sane upper
bound so we never OOM") + codebase — 10 MB is the existing, documented cutoff and
remains the correct outer bound.

### DEC-8: MARKDOWN — virtualize or keep the byte cap?
**Resolution:** Keep the byte cap for RENDERED markdown; document the rationale in
code. Streamdown exposes no block-windowing seam and reliable block-splitting of
markdown (nested tables/lists/fenced code) is not clean, so windowing rendered
markdown would risk correctness for little gain under the 10 MB bound. Markdown
RAW mode already delegates to `RawCodeView`, so it inherits DEC-1/DEC-2 windowing
for free.
**Basis:** codebase — `markdown/body.tsx` uses `Streamdown` (no windowing API) and
already has only a byte cap; the scope explicitly allows "document why + keep the
cap" when clean virtualization isn't feasible.

### DEC-9: Chunk size, prefetch margin, eager-load count (constants)
**Resolution:** `RAWCODE_CHUNK_LINES = 500`; IntersectionObserver
`rootMargin = '600px 0px'`; eager-highlight the first 2 chunks on mount (mirrors
PDF's eager first-pages). Reserved chunk height =
`chunkLineCount * linePx`, where `linePx ≈ 22` no-wrap; in wrap mode use a larger
per-line estimate (`~44`) since wrapped lines are taller — wrap-aware
`contain-intrinsic-size` (ITEM-4).
**Basis:** convention — matches the PDF observer's eager-first + rootMargin shape;
22 px is the value already documented in the current RawCodeView CSS
(`contain-intrinsic-size: auto 22px`).

### DEC-10: Do the retired 10k constants get renamed or removed?
**Resolution:** `MAX_LINES` (text) → replaced by `RAWCODE_MAX_LINES = 300_000`;
`MAX_ROWS` (DelimitedTable) → replaced by `DELIMITED_MAX_ROWS = 300_000` (raised,
not removed — see DEC-5); `MAX_ROWS` (XlsxBody) → replaced by `XLSX_MAX_ROWS =
200_000`. New windowing constants
(`RAWCODE_CHUNK_LINES`, observer margins) are module-local UPPER_SNAKE. Truncation
banners/testids (`file-rawcode-truncated-alert`, `file-delimited-truncated-alert`,
`file-xlsx-truncated-alert-*`) are KEPT (they now fire only at the raised
backstops), so existing selectors/e2e for the banner remain valid.
**Basis:** codebase — preserves the public testids/DOM contract while moving the
numeric thresholds.

### DEC-11: Which workspaces + gallery get changes?
**Resolution:** Source edits are single-source in `src-app/ui/src` (desktop
consumes via the `@/*` alias — no duplication). New gallery seeded surfaces are
added to the `ui` gallery only (`TableDemos.tsx` + `seededSurfaces.tsx`) and its
manifests regenerated; the desktop gallery is separate and does not reference the
file viewers, so it is untouched. `npm run check` must pass in BOTH workspaces.
**Basis:** codebase — verified `desktop/ui/tsconfig.json` alias + separate desktop
gallery.
