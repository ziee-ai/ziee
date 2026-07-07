import { useMemo } from 'react'
import { Alert } from '@/components/ui'
import { Table } from '@/components/ui/kit/table'
import type { TableColumn } from '@/components/ui/kit/table'
import { cn } from '@/lib/utils'

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

export function DelimitedTable({ text, delimiter }: { text: string; delimiter: string }) {
  // Parse + column/dataSource construction is the entire cost of this
  // component. Memoize on (text, delimiter) so panel re-renders for
  // unrelated reasons (resize, drawer, sibling state) don't re-parse the
  // whole file.
  const { columns, dataSource, truncated } = useMemo(() => {
    const { headers, rows, truncated } = parseDelimitedText(text, delimiter)
    const ROW_NUM_WIDTH = 56
    const COL_WIDTH = 240
    // Row-number gutter column. Width fits a 5-digit count (10,000 cap).
    // kit Table has no fixed/sticky column support, so the gutter scrolls
    // with the rest of the table.
    //
    // kit column render signature is (record, index) — no leading value arg.
    const rowNumberColumn: TableColumn<Record<string, string>> = {
      title: '#',
      dataIndex: '__rn',
      key: '__rn',
      width: ROW_NUM_WIDTH,
      align: 'right',
      render: (record: Record<string, string>) => (
        <span style={{ opacity: 0.5, fontVariantNumeric: 'tabular-nums' }}>
          {record.__rn}
        </span>
      ),
    }
    // Pre-compute column keys once — building 7k rows × N cols would
    // otherwise call `String(i)` 7k×N times in the dataSource loop.
    const colKeys = headers.map((_, i) => String(i))
    const dataColumns: TableColumn<Record<string, string>>[] = headers.map((h, i) => ({
      title: h || `Column ${i + 1}`,
      dataIndex: colKeys[i],
      key: colKeys[i],
      width: COL_WIDTH,
    }))
    const columns = [rowNumberColumn, ...dataColumns]
    const dataSource = rows.map((row, ri) => {
      const record: Record<string, string> = {
        key: String(ri),
        __rn: String(ri + 1),
      }
      for (let i = 0; i < colKeys.length; i++) {
        record[colKeys[i]] = row[i] ?? ''
      }
      return record
    })
    return { columns, dataSource, truncated }
  }, [text, delimiter])

  const virtualized = dataSource.length > VIRTUALIZE_ROW_THRESHOLD

  return (
    // A PLAIN (small) grid hugs its content so a 2-3 row table doesn't sit in a
    // tall empty box (no h-full). A VIRTUALIZED (large) grid needs a definite,
    // measurable scroll height or it renders its header but 0 data rows, so it
    // supplies its own bounded height rather than relying on the container (the
    // inline preview box only caps via max-height).
    <div
      className={cn(
        'flex flex-col w-full',
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
          majority — and most inline previews) render all rows and hug content. */}
      <Table
        virtualized={virtualized}
        columns={columns}
        dataSource={dataSource}
        rowKey="key"
        data-testid="file-delimited-table"
      />
    </div>
  )
}
