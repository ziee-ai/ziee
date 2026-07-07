import { useMemo, useRef, useState } from 'react'
import { Alert, message } from '@/components/ui'
import { Table } from '@/components/ui/kit/table'
import type { TableColumn } from '@/components/ui/kit/table'
import { detectNumericColumns } from '@/components/ui/kit/table-view-core'
import { cn } from '@/lib/utils'
import { ExpandableCell } from './ExpandableCell'
import { TabularToolbar } from './TabularToolbar'
import {
  type ExportColumn,
  type TabularRecord,
  downloadDelimited,
  exportFilename,
  rowsToDelimited,
} from './tableView'

/** Cap on rendered rows. Above this, the table is truncated to the
 *  first N and a banner offers Download for full content. The wider 8
 *  MB byte-cap at FilePanel still applies upstream — by the time we
 *  get here the file is already under that bound. `virtual` on the
 *  antd Table keeps row rendering cheap at this size. */
const MAX_ROWS = 10_000

/** Above this row count, switch the grid to row virtualization (needs a
 *  definite scroll-viewport height); at or below it, render a plain table so
 *  the rows are present in any container (content-sized inline previews
 *  included). Covers every inline preview and the vast majority of files. */
const VIRTUALIZE_ROW_THRESHOLD = 200

function parseDelimitedLine(line: string, delimiter: string): string[] {
  const fields: string[] = []
  let field = ''
  let inQuotes = false

  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (ch === '"') {
      if (inQuotes && line[i + 1] === '"') { field += '"'; i++ }
      else inQuotes = !inQuotes
    } else if (ch === delimiter && !inQuotes) {
      fields.push(field.trim())
      field = ''
    } else {
      field += ch
    }
  }
  fields.push(field.trim())
  return fields
}

function parseDelimitedText(text: string, delimiter: string): { headers: string[]; rows: string[][]; truncated: boolean } {
  const lines = text.split('\n').filter(l => l.trim() !== '')
  if (lines.length === 0) return { headers: [], rows: [], truncated: false }
  const headers = parseDelimitedLine(lines[0], delimiter)
  const dataLines = lines.slice(1)
  const truncated = dataLines.length > MAX_ROWS
  const rows = dataLines.slice(0, MAX_ROWS).map(l => parseDelimitedLine(l, delimiter))
  return { headers, rows, truncated }
}

export function DelimitedTable({ text, delimiter, fileName }: { text: string; delimiter: string; fileName?: string }) {
  // Parse + column/dataSource construction is the entire cost of this
  // component. Memoize on (text, delimiter) so panel re-renders for
  // unrelated reasons (resize, drawer, sibling state) don't re-parse the
  // whole file.
  const { columns, dataSource, truncated, exportColumns } = useMemo(() => {
    const { headers, rows, truncated } = parseDelimitedText(text, delimiter)
    const ROW_NUM_WIDTH = 56
    const COL_WIDTH = 240
    // Row-number gutter column. Width fits a 5-digit count (10,000 cap).
    // It is a `rowHeader` (clicking selects the whole row) and is excluded
    // from the column-chooser + copy/export.
    //
    // kit column render signature is (record, index) — no leading value arg.
    const rowNumberColumn: TableColumn<TabularRecord> = {
      title: '#',
      dataIndex: '__rn',
      key: '__rn',
      width: ROW_NUM_WIDTH,
      align: 'right',
      rowHeader: true,
      render: (record: TabularRecord) => (
        <span style={{ opacity: 0.5, fontVariantNumeric: 'tabular-nums' }}>
          {record.__rn}
        </span>
      ),
    }
    // Pre-compute column keys once — building 7k rows × N cols would
    // otherwise call `String(i)` 7k×N times in the dataSource loop.
    const colKeys = headers.map((_, i) => String(i))
    const dataSource: TabularRecord[] = rows.map((row, ri) => {
      const record: TabularRecord = {
        key: String(ri),
        __rn: String(ri + 1),
      }
      for (let i = 0; i < colKeys.length; i++) {
        record[colKeys[i]] = row[i] ?? ''
      }
      return record
    })
    // Numeric type detection (ITEM-7): a column is numeric when every sampled
    // non-empty cell parses as a number → right-align + tabular-nums (via the
    // kit's `numeric` flag). Non-numeric columns get click-to-expand cells.
    const numericKeys = detectNumericColumns(
      dataSource,
      colKeys.map(k => ({ key: k, dataIndex: k })),
    )
    const dataColumns: TableColumn<TabularRecord>[] = headers.map((h, i) => {
      const key = colKeys[i]
      const numeric = numericKeys.has(key)
      const col: TableColumn<TabularRecord> = {
        title: h || `Column ${i + 1}`,
        dataIndex: key,
        key,
        width: COL_WIDTH,
        sortable: true,
        hideable: true,
        numeric,
        ellipsis: numeric,
      }
      if (!numeric) {
        col.render = (record: TabularRecord) => (
          <ExpandableCell value={record[key] ?? ''} testid={`file-delimited-cell-${key}`} />
        )
      }
      return col
    })
    const columns = [rowNumberColumn, ...dataColumns]
    const exportColumns: ExportColumn[] = headers.map((h, i) => ({
      key: colKeys[i],
      title: h || `Column ${i + 1}`,
    }))
    return { columns, dataSource, truncated, exportColumns }
  }, [text, delimiter])

  const virtualized = dataSource.length > VIRTUALIZE_ROW_THRESHOLD

  // View state surfaced from the kit Table for the body-local toolbar.
  const viewRef = useRef<TabularRecord[]>(dataSource)
  const [viewCount, setViewCount] = useState(dataSource.length)
  const selectionRef = useRef('')
  const [scrollTo, setScrollTo] = useState<number | null>(null)
  // Currently-visible (non-gutter) column keys, reported by the kit Table, so
  // Export/Copy honour the column-chooser. Seeded with the full data set.
  const titleByKey = useMemo(
    () => new Map(exportColumns.map(c => [c.key, c.title])),
    [exportColumns],
  )
  const visibleKeysRef = useRef<string[]>(exportColumns.map(c => c.key))
  const activeColumns = (): ExportColumn[] =>
    visibleKeysRef.current.map(k => ({ key: k, title: titleByKey.get(k) ?? k }))

  const onJump = (rowNumber: number) => {
    const idx = viewRef.current.findIndex(r => r.__rn === String(rowNumber))
    if (idx < 0) {
      message.error(`Row ${rowNumber} is not in the current view`)
      return
    }
    // Force a change even when jumping to the same index twice.
    setScrollTo(null)
    requestAnimationFrame(() => setScrollTo(idx))
  }

  const onCopy = async () => {
    // selectionRef is already formula-neutralized by the kit (sanitizeClipboard);
    // the whole-view fallback goes through rowsToDelimited (also neutralized).
    const tsv = selectionRef.current || rowsToDelimited(viewRef.current, activeColumns(), '\t')
    try {
      await navigator.clipboard.writeText(tsv)
      message.success('Copied to clipboard')
    } catch {
      message.error('Failed to copy')
    }
  }

  const onExport = () => {
    const ext = delimiter === '\t' ? 'tsv' : 'csv'
    const csv = rowsToDelimited(viewRef.current, activeColumns(), delimiter)
    downloadDelimited(csv, exportFilename(fileName, ext), delimiter)
  }

  return (
    // A PLAIN (small) grid hugs its content so a 2-3 row table doesn't sit in a
    // tall empty box (no h-full). A VIRTUALIZED (large) grid needs a definite,
    // measurable scroll height or it renders its header but 0 data rows, so it
    // supplies its own bounded height rather than relying on the container (the
    // inline preview box only caps via max-height).
    // px-3 pt-3: inset the toolbar + grid from the panel body (which is
    // overflow-hidden with no padding of its own) — without it the toolbar sat
    // flush against the top/left edge and the jump input's focus ring was
    // clipped by the container's top edge.
    <div
      className={cn(
        'flex flex-col w-full px-3 pt-3',
        virtualized ? 'h-[min(360px,55vh)]' : 'max-h-[min(360px,55vh)]',
      )}
    >
      {truncated && (
        <Alert
          tone="warning"
          title={`Showing first ${MAX_ROWS.toLocaleString()} rows. Download the file to view all data.`}
          className="mb-2 flex-shrink-0"
          data-testid="file-delimited-truncated-alert"
        />
      )}
      {/* Virtualize only large grids. Row virtualization needs a definite,
          measurable scroll-viewport height (supplied by this root above); a
          plain table has no such dependency, so small grids (the overwhelming
          majority — and most inline previews) render all rows and hug content.
          The row-count readout + jump-to-row control ride the Table's own
          toolbar (toolbarExtra) so they share ONE row with filter + columns. */}
      <Table
        virtualized={virtualized}
        columns={columns}
        dataSource={dataSource}
        rowKey="key"
        sortable
        filterable
        resizable
        columnChooser
        toolbarExtra={
          <TabularToolbar
            testidPrefix="file-delimited"
            total={dataSource.length}
            viewCount={viewCount}
            onJump={onJump}
            onCopy={onCopy}
            onExport={onExport}
            exportLabel={delimiter === '\t' ? 'Export TSV' : 'Export CSV'}
          />
        }
        selectionMode="cell"
        sanitizeClipboard
        // Size columns to content (w-auto overrides the kit's default w-full):
        // with table-fixed + auto width the table hugs its content instead of
        // stretching a few short columns across the whole container. The
        // container still scrolls if the data is genuinely wider than it.
        className="w-auto table-auto"
        filterPlaceholder="Filter rows…"
        onViewChange={(rows, meta) => { viewRef.current = rows; setViewCount(rows.length); visibleKeysRef.current = meta.visibleColumns }}
        onSelectionChange={tsv => { selectionRef.current = tsv }}
        scrollToIndex={scrollTo}
        data-testid="file-delimited-table"
      />
    </div>
  )
}
