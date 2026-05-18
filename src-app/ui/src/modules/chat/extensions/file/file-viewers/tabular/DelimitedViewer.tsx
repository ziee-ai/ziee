import { Spin, Table, Alert } from 'antd'
import type { TableColumnsType } from 'antd'
import { Stores } from '@/core/stores'
import type { FileViewRendererProps } from '../../types'

const MAX_ROWS = 100

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

function DelimitedTable({ text, delimiter }: { text: string; delimiter: string }) {
  const { headers, rows, truncated } = parseDelimitedText(text, delimiter)
  const columns: TableColumnsType<Record<string, string>> = headers.map((h, i) => ({
    title: h || `Column ${i + 1}`,
    dataIndex: String(i),
    key: String(i),
    ellipsis: true,
  }))
  const dataSource = rows.map((row, ri) => {
    const record: Record<string, string> = { key: String(ri) }
    headers.forEach((_, i) => { record[String(i)] = row[i] ?? '' })
    return record
  })
  return (
    <div className="px-2">
      {truncated && (
        <Alert
          message={`Showing first ${MAX_ROWS} rows. Download the file to view all data.`}
          type="warning"
          showIcon
          className="mb-2"
        />
      )}
      <Table
        columns={columns}
        dataSource={dataSource}
        size="small"
        scroll={{ x: true, y: 'calc(100vh - 220px)' }}
        pagination={false}
      />
    </div>
  )
}

export function CsvViewer({ file }: FileViewRendererProps) {
  const { fileTextContents } = Stores.Chat.FileStore
  const content = fileTextContents.get(file.id) ?? null
  if (content === null) Stores.Chat.FileStore.getFileTextContent(file.id, file)
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  return <DelimitedTable text={content} delimiter="," />
}

export function TsvViewer({ file }: FileViewRendererProps) {
  const { fileTextContents } = Stores.Chat.FileStore
  const content = fileTextContents.get(file.id) ?? null
  if (content === null) Stores.Chat.FileStore.getFileTextContent(file.id, file)
  if (content === null) {
    return <div className="flex items-center justify-center h-full"><Spin /></div>
  }
  return <DelimitedTable text={content} delimiter={'\t'} />
}
