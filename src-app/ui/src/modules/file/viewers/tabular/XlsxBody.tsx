import { useState, useEffect, useMemo, useRef } from 'react'
import { Spin, Alert, Text, message } from '@/components/ui'
import { Tabs } from '@/components/ui/kit/tabs'
import { Table } from '@/components/ui/kit/table'
import type { TableColumn } from '@/components/ui/kit/table'
import { detectNumericColumns } from '@/components/ui/kit/table-view-core'
import { Stores } from '@/core/stores'
import { cn } from '@/lib/utils'
import type { FileViewerSlotProps } from '../../types/viewer'
import { ExpandableCell } from './ExpandableCell'
import { TabularToolbar } from './TabularToolbar'
import {
  type ExportColumn,
  type TabularRecord,
  downloadBlob,
  exportFilename,
  rowsToDelimited,
  viewToXlsxBlob,
} from './tableView'

/** Above this row count a sheet switches to row virtualization (needs a definite
 *  scroll height); at/below it renders a plain table that hugs its content, so a
 *  2-3 row sheet doesn't sit in a tall empty box. Mirrors DelimitedTable. */
const VIRTUALIZE_ROW_THRESHOLD = 200

/** Per-sheet cap on rendered rows. Above this, each sheet is truncated
 *  to the first N and a banner offers Download. The wider 10 MB
 *  byte-cap at FilePanel still applies upstream. */
const MAX_ROWS = 10_000

/** Per-data-column width — same value DelimitedTable uses for the
 *  matching CSV/TSV view. Caps wide cells; the table scrolls horizontally
 *  when the sum exceeds the container width. */
const ROW_NUM_WIDTH = 56
const COL_WIDTH = 240

export interface Sheet {
  name: string
  headers: string[]
  rows: string[][]
  truncated: boolean
}

// One sheet's grid — owns its own view state (sort/filter/selection) + toolbar,
// so switching tabs keeps each sheet independent. Mirrors DelimitedTable.
// Exported for the gallery (renderable from a plain `sheet` prop, no binary).
export function XlsxSheet({ sheet, fileName }: { sheet: Sheet; fileName?: string }) {
  const { columns, dataSource, exportColumns } = useMemo(() => {
    const colKeys = sheet.headers.map((_, i) => String(i))
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
    const dataSource: TabularRecord[] = sheet.rows.map((row, ri) => {
      const record: TabularRecord = { key: String(ri), __rn: String(ri + 1) }
      for (let i = 0; i < colKeys.length; i++) {
        record[colKeys[i]] = String(row[i] ?? '')
      }
      return record
    })
    const numericKeys = detectNumericColumns(
      dataSource,
      colKeys.map(k => ({ key: k, dataIndex: k })),
    )
    const dataColumns: TableColumn<TabularRecord>[] = sheet.headers.map((h, i) => {
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
          <ExpandableCell value={record[key] ?? ''} testid={`file-xlsx-cell-${sheet.name}-${key}`} />
        )
      }
      return col
    })
    const columns = [rowNumberColumn, ...dataColumns]
    const exportColumns: ExportColumn[] = sheet.headers.map((h, i) => ({
      key: colKeys[i],
      title: h || `Column ${i + 1}`,
    }))
    return { columns, dataSource, exportColumns }
  }, [sheet])

  const virtualized = dataSource.length > VIRTUALIZE_ROW_THRESHOLD
  const viewRef = useRef<TabularRecord[]>(dataSource)
  const [viewCount, setViewCount] = useState(dataSource.length)
  const selectionRef = useRef('')
  const [scrollTo, setScrollTo] = useState<number | null>(null)

  const onJump = (rowNumber: number) => {
    const idx = viewRef.current.findIndex(r => r.__rn === String(rowNumber))
    if (idx < 0) {
      message.error(`Row ${rowNumber} is not in the current view`)
      return
    }
    setScrollTo(null)
    requestAnimationFrame(() => setScrollTo(idx))
  }

  const onCopy = async () => {
    const tsv = selectionRef.current || rowsToDelimited(viewRef.current, exportColumns, '\t')
    try {
      await navigator.clipboard.writeText(tsv)
      message.success('Copied to clipboard')
    } catch {
      message.error('Failed to copy')
    }
  }

  const onExport = async () => {
    try {
      const blob = await viewToXlsxBlob(viewRef.current, exportColumns, sheet.name)
      downloadBlob(blob, exportFilename(fileName, 'xlsx'))
    } catch {
      message.error('Failed to export sheet')
    }
  }

  return (
    // Plain (small) sheet hugs its content; a virtualized (large) sheet
    // supplies its own definite scroll height (see DelimitedTable).
    <div
      className={cn(
        'flex flex-col w-full',
        virtualized ? 'h-[min(360px,55vh)]' : 'max-h-[min(360px,55vh)]',
      )}
    >
      {sheet.truncated && (
        <Alert
          tone="warning"
          title={`Showing first ${MAX_ROWS.toLocaleString()} rows. Download the file to view all data.`}
          className="mb-2 flex-shrink-0"
          data-testid={`file-xlsx-truncated-alert-${sheet.name}`}
        />
      )}
      <TabularToolbar
        testidPrefix={`file-xlsx-${sheet.name}`}
        total={dataSource.length}
        viewCount={viewCount}
        onJump={onJump}
        onCopy={onCopy}
        onExport={onExport}
        exportLabel="Export XLSX"
      />
      {/* The virtualized Table owns its own OverlayScrollbars scroll box. */}
      <Table
        virtualized={virtualized}
        columns={columns}
        dataSource={dataSource}
        rowKey="key"
        sortable
        filterable
        resizable
        columnChooser
        selectionMode="cell"
        filterPlaceholder="Filter rows…"
        onViewChange={rows => { viewRef.current = rows; setViewCount(rows.length) }}
        onSelectionChange={tsv => { selectionRef.current = tsv }}
        scrollToIndex={scrollTo}
        data-testid={`file-xlsx-table-${sheet.name}`}
      />
    </div>
  )
}

export function XlsxBody(props: FileViewerSlotProps) {
  // XLSX is not inline-capable (binary parse + heavy bundle). Guard for
  // type-narrowing; chat dispatcher won't reach here for source-shaped props.
  const file = 'file' in props ? props.file : null
  const { fileBinaryContents } = Stores.File
  const fileBinaryContent = file
    ? (fileBinaryContents.get(file.id) ?? null)
    : null
  if (file && fileBinaryContent === null) {
    Stores.File.getFileBinaryContent(file.id, file)
  }
  const [sheets, setSheets] = useState<Sheet[]>([])
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)

  useEffect(() => {
    if (!fileBinaryContent) return
    let cancelled = false
    setLoadError(null)
    import('xlsx')
      .then(XLSX => {
        try {
          // Wrap the ArrayBuffer in a Uint8Array view — xlsx 0.18's
          // `type: 'array'` expects a byte array (indexed view), not
          // a raw ArrayBuffer. Passing the bare buffer mis-interprets
          // byteLength → enormous internal allocations.
          //
          // `sheetRows: MAX_ROWS + 1` stops parsing past our row cap
          // so large sheets don't materialize all rows in memory
          // before our `slice(0, MAX_ROWS)` discards them.
          const wb = XLSX.read(new Uint8Array(fileBinaryContent), {
            type: 'array',
            sheetRows: MAX_ROWS + 1,
          })
          const parsed = wb.SheetNames.slice(0, 10).map(name => {
            const ws = wb.Sheets[name]
            const data = XLSX.utils.sheet_to_json<string[]>(ws, { header: 1, defval: '' })
            const headers = (data[0] as string[]) ?? []
            const dataRows = (data.slice(1) as string[][])
            const truncated = dataRows.length > MAX_ROWS
            const rows = dataRows.slice(0, MAX_ROWS)
            return { name, headers, rows, truncated }
          })
          if (!cancelled) {
            setSheets(parsed)
            setLoading(false)
          }
        } catch (err) {
          if (!cancelled) {
            setLoadError(err instanceof Error ? err.message : 'Failed to parse spreadsheet')
            setLoading(false)
          }
        }
      })
      .catch(err => {
        // Without this catch, a dynamic-import failure (e.g., Vite 504 mid-
        // optimization) leaves loading=true forever and the body shows a
        // spinner with no recovery path.
        if (!cancelled) {
          setLoadError(err instanceof Error ? err.message : 'Failed to load xlsx parser')
          setLoading(false)
        }
      })
    return () => { cancelled = true }
  }, [fileBinaryContent])

  if (!file) return null

  if (loadError) {
    return (
      <div className="flex flex-col items-center justify-center py-8 gap-2" data-testid="file-xlsx-error">
        <Text type="danger">Failed to load spreadsheet preview</Text>
        <Text type="secondary" className="text-xs">{loadError}</Text>
      </div>
    )
  }

  if (!fileBinaryContent || loading) {
    return <div className="flex items-center justify-center py-8"><Spin label="Loading" /></div>
  }

  if (sheets.length === 0) {
    return <div className="flex items-center justify-center py-8"><Text type="secondary">No data found</Text></div>
  }

  if (sheets.length === 1) {
    // A single-sheet workbook renders no tab bar (nothing to switch between),
    // but the viewer root still carries `file-xlsx-tabs` so the same selector
    // locates the xlsx surface whether or not tabs are shown. (The kit Tabs
    // root already forwards this testid in the multi-sheet branch below.)
    return (
      <div className="flex flex-col w-full" data-testid="file-xlsx-tabs">
        <XlsxSheet sheet={sheets[0]} fileName={file.filename} />
      </div>
    )
  }

  return (
    // Multi-sheet: the Tabs `fill` layout needs a definite height for the grid
    // panel, so cap at the same bounded height (small multi-sheet books keep the
    // tab chrome; the single-sheet path above hugs content).
    <div className="flex flex-col h-[min(360px,55vh)] w-full">
      <Tabs
        data-testid="file-xlsx-tabs"
        fill
        scrollX
        className="flex-1 min-h-0"
        tabStripClassName="[justify-content:safe_center] px-3 gap-1"
        items={sheets.map(sheet => ({
          key: sheet.name,
          label: sheet.name,
          children: (
            <div className="flex flex-col min-h-0 overflow-hidden h-full">
              <XlsxSheet sheet={sheet} fileName={file.filename} />
            </div>
          ),
        }))}
      />
    </div>
  )
}
