import { useState, useEffect, useRef } from 'react'
import { Spin, Typography, Table, Tabs, Alert } from 'antd'
import type { TableColumnsType } from 'antd'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types/viewer'

const { Text } = Typography

/** Per-sheet cap on rendered rows. Above this, each sheet is truncated
 *  to the first N and a banner offers Download. The wider 10 MB
 *  byte-cap at FilePanel still applies upstream. `virtual` on the
 *  antd Table keeps row rendering cheap at this size. */
const MAX_ROWS = 10_000

/** Per-data-column width — same value DelimitedTable uses for the
 *  matching CSV/TSV view. Caps wide cells, total column width drives
 *  horizontal scroll. Sum of (row-num + N*COL_WIDTH) flows into
 *  `scroll.x` for antd's virtual mode. */
const ROW_NUM_WIDTH = 56
const COL_WIDTH = 240

/** See DelimitedTable for the same constant + reasoning. */
const TABLE_HEADER_PX = 48

/** Antd Tabs nav row height (default `large` size). Used to subtract
 *  from the measured wrap when there's more than one sheet (the nav
 *  takes ~46px out of the available space for the table body). */
const TABS_NAV_PX = 46

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

  // Shared measured height for all sheet tables. Hoisted above the
  // early-return branches below so the hook count is stable across
  // loading / error / loaded renders (React's Rules of Hooks
  // requires the SAME hooks in the SAME order every render — adding
  // hooks below conditional returns trips "Rendered more hooks
  // than during the previous render"). See DelimitedTable for full
  // reasoning on the height-measurement approach itself.
  const wrapRef = useRef<HTMLDivElement>(null)
  const [bodyHeight, setBodyHeight] = useState<number>(600)
  // Multi-sheet workbooks render in a Tabs; subtract the tab nav as
  // well so the body fits inside the tab pane. Single-sheet only
  // pays the table-header cost.
  const sheetsCount = sheets.length
  const chromePx = TABLE_HEADER_PX + (sheetsCount > 1 ? TABS_NAV_PX : 0)

  useEffect(() => {
    if (!wrapRef.current) return
    const ro = new ResizeObserver(entries => {
      for (const entry of entries) {
        const h = Math.floor(entry.contentRect.height) - chromePx
        if (h > 0) setBodyHeight(h)
      }
    })
    ro.observe(wrapRef.current)
    return () => ro.disconnect()
  }, [chromePx])

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
    return <div className="flex items-center justify-center py-8"><Spin /></div>
  }

  if (sheets.length === 0) {
    return <div className="flex items-center justify-center py-8"><Text type="secondary">No data found</Text></div>
  }

  const renderSheet = (sheet: { headers: string[]; rows: string[][]; truncated: boolean }) => {
    // Row-number gutter column — matches the CSV/TSV view for
    // visual consistency. Fixed-left so it stays anchored when
    // horizontally scrolling wide sheets.
    const rowNumberColumn: TableColumnsType<Record<string, string>>[number] = {
      title: '#',
      dataIndex: '__rn',
      key: '__rn',
      width: ROW_NUM_WIDTH,
      fixed: 'left',
      align: 'right',
      render: (v: string) => (
        <span style={{ opacity: 0.5, fontVariantNumeric: 'tabular-nums' }}>
          {v}
        </span>
      ),
    }
    // Pre-compute column keys once — calling String(i) per-cell
    // adds up over 10k rows × N cols.
    const colKeys = sheet.headers.map((_, i) => String(i))
    const dataColumns: TableColumnsType<Record<string, string>> = sheet.headers.map((h, i) => ({
      title: h || `Column ${i + 1}`,
      dataIndex: colKeys[i],
      key: colKeys[i],
      width: COL_WIDTH,
      ellipsis: { showTitle: true },
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
    // Explicit pixel width for scroll.x — antd's virtual mode
    // partially disables when scroll.x is the boolean `true`.
    const scrollX = ROW_NUM_WIDTH + sheet.headers.length * COL_WIDTH
    return (
      <div className="flex flex-col h-full w-full px-2">
        {sheet.truncated && (
          <Alert
            title={`Showing first ${MAX_ROWS.toLocaleString()} rows. Download the file to view all data.`}
            type="warning"
            showIcon
            className="mb-2 flex-shrink-0"
          />
        )}
        <div className="flex-1 min-h-0 w-full">
          <Table
            columns={columns}
            dataSource={dataSource}
            size="small"
            scroll={{ x: scrollX, y: bodyHeight }}
            pagination={false}
            virtual
          />
        </div>
      </div>
    )
  }

  if (sheets.length === 1) {
    return (
      <div ref={wrapRef} className="flex flex-col h-full w-full">
        {renderSheet(sheets[0])}
      </div>
    )
  }

  // Tailwind arbitrary-selector overrides on antd Tabs internals:
  //
  // - `flex flex-1 min-h-0` on `.ant-tabs-content-holder` /
  //   `.ant-tabs-content` makes the body section consume the
  //   remaining vertical space inside the Tabs root (default
  //   behavior collapses to natural content height).
  // - `min-w-0` + `overflow-hidden` on every layer constrains the
  //   table to the holder's width — without these, the flex
  //   children's default `min-width: auto` lets the table expand
  //   past container width, which suppresses the antd Table's own
  //   `.ant-table-body { overflow-x: auto }` scrollbar (nothing to
  //   scroll if you've already pushed the parent wide).
  // - `h-full` on `.ant-tabs-tabpane` so each sheet's renderSheet
  //   wrapper gets a definite height to fill via its own `h-full`.
  const tabsFullHeight =
    'flex-1 min-h-0 min-w-0 flex flex-col overflow-hidden ' +
    '[&_.ant-tabs-content-holder]:flex [&_.ant-tabs-content-holder]:flex-1 [&_.ant-tabs-content-holder]:min-h-0 [&_.ant-tabs-content-holder]:min-w-0 [&_.ant-tabs-content-holder]:overflow-hidden ' +
    '[&_.ant-tabs-content]:flex [&_.ant-tabs-content]:flex-1 [&_.ant-tabs-content]:min-h-0 [&_.ant-tabs-content]:min-w-0 [&_.ant-tabs-content]:overflow-hidden ' +
    '[&_.ant-tabs-tabpane]:h-full [&_.ant-tabs-tabpane]:min-w-0 [&_.ant-tabs-tabpane]:overflow-hidden'

  return (
    <div ref={wrapRef} className="flex flex-col h-full w-full">
      <Tabs
        // `type="card"` swaps the default underline for the
        // chrome-tab / browser-tab style with each tab as its own
        // card surface — clearer affordance that each tab is its
        // own sheet.
        type="card"
        size="small"
        className={tabsFullHeight}
        items={sheets.map(sheet => ({
          key: sheet.name,
          label: sheet.name,
          children: renderSheet(sheet),
        }))}
      />
    </div>
  )
}
