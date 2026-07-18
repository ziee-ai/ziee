/**
 * Standalone kit-Table + tabular-viewer demos, shared by the browse-view story
 * cases (data.story.tsx) AND the isolated `?surface=` seeded entries the
 * interactive e2e drives (the browse canvas mounts every story at once, so
 * click-based specs run against these focused single-surface renders instead).
 * Store-free — safe to render standalone.
 */
import { useState } from 'react'
import { Button, Table } from '@ziee/kit'
import type { TableColumn } from '@ziee/kit'
import type { File as FileEntity } from '@/api-client/types'
import { DelimitedTable } from '@/modules/file/viewers/tabular/DelimitedTable'
import { DelimitedHeader } from '@/modules/file/viewers/tabular/header'
import { XlsxSheet } from '@/modules/file/viewers/tabular/XlsxBody'
import { RawCodeView } from '@/modules/file/viewers/shared/RawCodeView'

interface ARow {
  id: string
  name: string
  qty: string
  note: string
}
const LONG =
  'This is a deliberately long cell value that must be clipped and expandable via a popover in the tabular viewer'
const actionRows: ARow[] = [
  { id: '1', name: 'Banana', qty: '10', note: 'A short note' },
  { id: '2', name: 'apple', qty: '2', note: LONG },
  { id: '3', name: 'Cherry', qty: '30', note: 'Another note' },
]
const actionColumns: TableColumn<ARow>[] = [
  { key: 'name', title: 'Name', dataIndex: 'name', hideable: true },
  { key: 'qty', title: 'Qty', dataIndex: 'qty', hideable: true },
  { key: 'note', title: 'Note', dataIndex: 'note', ellipsis: true, hideable: true },
]

export function TableActionsDemo() {
  return (
    <div className="w-[34rem] max-w-full p-2">
      <Table
        data-testid="g-table-actions"
        columns={actionColumns}
        dataSource={actionRows}
        rowKey="id"
        sortable
        filterable
        resizable
        columnChooser
        detectNumericColumns
        selectionMode="cell"
        filterPlaceholder="Filter rows…"
      />
    </div>
  )
}

export function TableScrollDemo() {
  const [scrollTo, setScrollTo] = useState<number | null>(null)
  const rows = Array.from({ length: 500 }, (_, i) => ({
    id: String(i),
    name: `Row ${i}`,
    value: String(i * 3),
  }))
  const cols: TableColumn<{ id: string; name: string; value: string }>[] = [
    { key: 'name', title: 'Name', dataIndex: 'name' },
    { key: 'value', title: 'Value', dataIndex: 'value', numeric: true },
  ]
  return (
    <div className="w-96 max-w-full flex flex-col gap-2 p-2">
      <Button data-testid="g-table-scroll-btn" onClick={() => setScrollTo(400)}>
        Scroll to row 400
      </Button>
      {/* Drive the scroll-box height via the Table's own `maxHeight` prop (a
          single source of truth) — an external fixed-height wrapper doesn't
          constrain the inner OverlayScrollbars viewport, so the table would
          overflow it. */}
      <Table
        data-testid="g-table-scroll"
        columns={cols}
        dataSource={rows}
        rowKey="id"
        virtualized
        maxHeight="16rem"
        scrollToIndex={scrollTo}
      />
    </div>
  )
}

const CSV_TEXT = [
  'Name,Qty,Note',
  'Banana,10,A short note',
  `apple,2,${LONG}`,
  'Cherry,30,Another note',
  'Date,7,Yet another',
].join('\n')

export function DelimitedViewerDemo() {
  return (
    <div className="w-[36rem] max-w-full p-2">
      <DelimitedTable text={CSV_TEXT} delimiter="," fileName="data.csv" />
    </div>
  )
}

// A CSV File entity for the header-inclusive demo. Minimal but well-shaped so
// `DelimitedHeader` (which needs `{ file }`) type-checks; text_page_count:0 hides
// the raw toggle (this isolated demo renders the table directly, not via the
// body's mode path), leaving the whole-file Copy + the view-aware Export /
// Copy-selection actions this surface exists to exercise.
const csvFile = {
  id: 'gallery0-csv0-4000-8000-000000000001',
  user_id: 'gallery-user',
  created_by: 'gallery-user',
  filename: 'data.csv',
  file_size: CSV_TEXT.length,
  mime_type: 'text/csv',
  checksum: 'sha256:gallery-csv',
  blob_version_id: 'gallery-csv-v1',
  current_version_id: 'gallery-csv-v1',
  version: 1,
  has_thumbnail: false,
  preview_page_count: 0,
  text_page_count: 0,
  processing_metadata: {},
  created_at: '2026-05-03T08:00:00.000000Z',
  updated_at: '2026-05-03T08:00:00.000000Z',
} satisfies FileEntity

/** The real DelimitedTable UNDER the real file-viewer header actions, mirroring
 *  the shell's header-above-body layout. Drives the view-aware Export /
 *  Copy-selection hookup (`DelimitedHeader` ↔ body via FileStore.fileTabularView)
 *  without the async `/text` load path — the table renders straight from `text`. */
export function DelimitedViewerWithHeaderDemo() {
  return (
    <div className="w-[36rem] max-w-full flex flex-col gap-2 p-2">
      <div className="flex items-center justify-end">
        <DelimitedHeader file={csvFile} />
      </div>
      <DelimitedTable text={CSV_TEXT} delimiter="," fileName="data.csv" fileId={csvFile.id} />
    </div>
  )
}

export function XlsxViewerDemo() {
  return (
    <div className="w-[36rem] max-w-full p-2">
      <XlsxSheet
        fileName="book.xlsx"
        sheet={{
          name: 'Sheet1',
          headers: ['Name', 'Qty', 'Note'],
          rows: [
            ['Banana', '10', 'A short note'],
            ['apple', '2', 'Another'],
            ['Cherry', '30', 'Third'],
          ],
          truncated: false,
        }}
      />
    </div>
  )
}

// ── Large-file demos (file-viewer-virtualization) ────────────────────────────
// Exercise the lifted per-viewer caps + windowed render paths: the tabular grid
// row-virtualizes a >10k-row dataset (sort/filter span the whole set), and the
// raw-code view chunk-windows its Shiki highlight over thousands of lines. Kept
// modest (a few thousand rows/lines) so the gallery crawl stays fast while still
// crossing the virtualization threshold + multiple highlight chunks.
const LARGE_CSV_TEXT = (() => {
  const rows = ['id,name,value,note']
  for (let i = 1; i <= 12_000; i++) rows.push(`${i},Row ${i},${i * 3},note-${i}`)
  return rows.join('\n')
})()

export function LargeDelimitedViewerDemo() {
  return (
    <div className="w-[40rem] max-w-full p-2">
      <DelimitedTable text={LARGE_CSV_TEXT} delimiter="," fileName="large.csv" />
    </div>
  )
}

const LARGE_TEXT = (() => {
  const lines: string[] = []
  for (let i = 1; i <= 3_000; i++) {
    lines.push(`line ${i}: const value_${i} = compute(${i}) // marker-${i}`)
  }
  return lines.join('\n')
})()

export function LargeRawCodeViewDemo() {
  return (
    <div className="w-[40rem] h-[24rem] max-w-full p-2">
      <RawCodeView text={LARGE_TEXT} filename="large.ts" />
    </div>
  )
}
