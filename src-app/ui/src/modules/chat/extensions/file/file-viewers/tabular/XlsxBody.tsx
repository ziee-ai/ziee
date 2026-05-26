import { useState, useEffect } from 'react'
import { Spin, Typography, Table, Tabs, Alert } from 'antd'
import type { TableColumnsType } from 'antd'
import { Stores } from '@/core/stores'
import type { FileViewerSlotProps } from '../../types'

const { Text } = Typography

const MAX_ROWS = 100

export function XlsxBody({ file }: FileViewerSlotProps) {
  const { fileBinaryContents } = Stores.Chat.FileStore
  const fileBinaryContent = fileBinaryContents.get(file.id) ?? null
  if (fileBinaryContent === null) Stores.Chat.FileStore.getFileBinaryContent(file.id, file)
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
          const wb = XLSX.read(fileBinaryContent, { type: 'array' })
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
    const columns: TableColumnsType<Record<string, string>> = sheet.headers.map((h, i) => ({
      title: h || `Column ${i + 1}`,
      dataIndex: String(i),
      key: String(i),
      ellipsis: true,
    }))
    const dataSource = sheet.rows.map((row, ri) => {
      const record: Record<string, string> = { key: String(ri) }
      sheet.headers.forEach((_, i) => { record[String(i)] = String(row[i] ?? '') })
      return record
    })
    return (
      <div className="px-2">
        {sheet.truncated && (
          <Alert
            title={`Showing first ${MAX_ROWS} rows. Download the file to view all data.`}
            type="warning"
            showIcon
            className="mb-2"
          />
        )}
        <Table
          columns={columns}
          dataSource={dataSource}
          size="small"
          scroll={{ x: true, y: 'calc(100vh - 260px)' }}
          pagination={false}
        />
      </div>
    )
  }

  if (sheets.length === 1) return renderSheet(sheets[0])

  return (
    <Tabs
      items={sheets.map(sheet => ({
        key: sheet.name,
        label: sheet.name,
        children: renderSheet(sheet),
      }))}
    />
  )
}
