/**
 * Standalone kit-Table + tabular-viewer demos, shared by the browse-view story
 * cases (data.story.tsx) AND the isolated `?surface=` seeded entries the
 * interactive e2e drives (the browse canvas mounts every story at once, so
 * click-based specs run against these focused single-surface renders instead).
 * Store-free — safe to render standalone.
 */
import { useState } from 'react'
import { Button, Table } from '@/components/ui'
import type { TableColumn } from '@/components/ui'
import { DelimitedTable } from '@/modules/file/viewers/tabular/DelimitedTable'
import { XlsxSheet } from '@/modules/file/viewers/tabular/XlsxBody'

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
