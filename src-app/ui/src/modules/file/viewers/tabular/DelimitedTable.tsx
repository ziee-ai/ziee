import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Alert, message } from '@/components/ui'
import { Table } from '@/components/ui/kit/table'
import type { TableColumn } from '@/components/ui/kit/table'
import { detectNumericColumns } from '@/components/ui/kit/table-view-core'
import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'
import { ExpandableCell } from './ExpandableCell'
import { TabularToolbar } from './TabularToolbar'
import type { ExportColumn, TabularRecord } from './tableView'
import { DELIMITED_MAX_ROWS, parseDelimitedText } from './parse'

/** Above this row count, switch the grid to row virtualization (needs a
 *  definite scroll-viewport height); at or below it, render a plain table so
 *  the rows are present in any container (content-sized inline previews
 *  included). Covers every inline preview and the vast majority of files. */
const VIRTUALIZE_ROW_THRESHOLD = 200

export function DelimitedTable({ text, delimiter, fileName, fileId }: { text: string; delimiter: string; fileName?: string; fileId?: string }) {
  // Parse + column/dataSource construction is the entire cost of this
  // component. Memoize on (text, delimiter) so panel re-renders for
  // unrelated reasons (resize, drawer, sibling state) don't re-parse the
  // whole file.
  const { columns, dataSource, truncated, exportColumns } = useMemo(() => {
    const { headers, rows, truncated } = parseDelimitedText(text, delimiter)
    const ROW_NUM_WIDTH = 56
    const COL_WIDTH = 240
    // Row-number gutter column. Width fits a 6-digit count (300k cap).
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
  // Visible (non-gutter) columns in display order, honouring the chooser.
  const activeColumns = useCallback(
    (): ExportColumn[] =>
      visibleKeysRef.current.map(k => ({ key: k, title: titleByKey.get(k) ?? k })),
    [titleByKey],
  )

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

  // Publish the current view snapshot for the file-viewer header's view-aware
  // Export / Copy-selection actions (see DelimitedHeader). No-op in the
  // inline/chat context (no file id) — there the header isn't rendered.
  // selectionRef is already formula-neutralized by the kit (sanitizeClipboard).
  const publishView = useCallback(() => {
    if (!fileId) return
    Stores.File.setFileTabularView(fileId, {
      rows: viewRef.current,
      columns: activeColumns(),
      delimiter,
      fileName,
      selectionTsv: selectionRef.current,
    })
  }, [fileId, activeColumns, delimiter, fileName])

  // On mount + whenever the parsed data changes (new file/text), reset the view
  // refs to the fresh full parse BEFORE publishing — otherwise the snapshot would
  // briefly carry the previous file's rows/selection until the kit's onViewChange
  // re-fires. Keeps the header's actions correct for the file actually shown.
  useEffect(() => {
    viewRef.current = dataSource
    visibleKeysRef.current = exportColumns.map(c => c.key)
    selectionRef.current = ''
    publishView()
  }, [publishView, dataSource, exportColumns])

  // Drop the published snapshot when the table unmounts (panel close / switch to
  // raw view) so the header's Export / Copy-selection disable rather than act on
  // a view that is no longer rendered.
  useEffect(() => {
    if (!fileId) return
    return () => Stores.File.clearFileTabularView(fileId)
  }, [fileId])

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
          title={`Showing first ${DELIMITED_MAX_ROWS.toLocaleString()} rows. Download the file to view all data.`}
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
        onViewChange={(rows, meta) => { viewRef.current = rows; setViewCount(rows.length); visibleKeysRef.current = meta.visibleColumns; publishView() }}
        onSelectionChange={tsv => { selectionRef.current = tsv; publishView() }}
        scrollToIndex={scrollTo}
        data-testid="file-delimited-table"
      />
    </div>
  )
}
