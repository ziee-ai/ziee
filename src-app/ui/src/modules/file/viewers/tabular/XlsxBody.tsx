import { useState, useEffect } from 'react'
import { Spin, Alert, Text } from '@/components/ui'
import { Tabs } from '@/components/ui/kit/tabs'
import { Table } from '@/components/ui/kit/table'
import type { TableColumn } from '@/components/ui/kit/table'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'

/** Per-sheet cap on rendered rows. Above this, each sheet is truncated
 *  to the first N and a banner offers Download. The wider 10 MB
 *  byte-cap at FilePanel still applies upstream. */
const MAX_ROWS = 10_000

/** Per-data-column width — same value DelimitedTable uses for the
 *  matching CSV/TSV view. Caps wide cells; the table scrolls horizontally
 *  when the sum exceeds the container width. */
const ROW_NUM_WIDTH = 56
const COL_WIDTH = 240

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
  const [sheets, setSheets] = useState<{ name: string; headers: string[]; rows: string[][]; truncated: boolean }[]>([])
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
      <div className="flex flex-col items-center justify-center py-8 gap-2">
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

  const renderSheet = (sheet: { headers: string[]; rows: string[][]; truncated: boolean }) => {
    // Row-number gutter column — matches the CSV/TSV view for
    // visual consistency. kit Table has no fixed/sticky column support,
    // so the gutter scrolls with the rest of the sheet.
    //
    // kit column render signature is (record, index) — no leading value arg.
    const colKeys = sheet.headers.map((_, i) => String(i))
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
    const dataColumns: TableColumn<Record<string, string>>[] = sheet.headers.map((h, i) => ({
      title: h || `Column ${i + 1}`,
      dataIndex: colKeys[i],
      key: colKeys[i],
      width: COL_WIDTH,
    }))
    const columns = [rowNumberColumn, ...dataColumns]
    const dataSource = sheet.rows.map((row, ri) => {
      const record: Record<string, string> = {
        key: String(ri),
        __rn: String(ri + 1),
      }
      for (let i = 0; i < colKeys.length; i++) {
        record[colKeys[i]] = String(row[i] ?? '')
      }
      return record
    })
    return (
      <div className="flex flex-col h-full w-full px-2">
        {sheet.truncated && (
          <Alert
            tone="warning"
            title={`Showing first ${MAX_ROWS.toLocaleString()} rows. Download the file to view all data.`}
            className="mb-2 flex-shrink-0"
          />
        )}
        <div className="flex-1 min-h-0 overflow-auto w-full">
          <Table
            columns={columns}
            dataSource={dataSource}
            rowKey="key"
          />
        </div>
      </div>
    )
  }

  if (sheets.length === 1) {
    return (
      <div className="flex flex-col h-full w-full overflow-auto">
        {renderSheet(sheets[0])}
      </div>
    )
  }

  return (
    <div className="flex flex-col h-full w-full">
      <Tabs
        className="flex-1 min-h-0 flex flex-col overflow-hidden"
        items={sheets.map(sheet => ({
          key: sheet.name,
          label: sheet.name,
          children: (
            <div className="flex flex-col min-h-0 overflow-hidden h-full">
              {renderSheet(sheet)}
            </div>
          ),
        }))}
      />
    </div>
  )
}
