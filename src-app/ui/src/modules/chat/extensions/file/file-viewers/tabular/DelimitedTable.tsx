import { useMemo } from 'react'
import { Table, Alert } from 'antd'
import type { TableColumnsType } from 'antd'

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

export function DelimitedTable({ text, delimiter }: { text: string; delimiter: string }) {
  // Parse + column/dataSource construction is the entire cost of this
  // component. Memoize on (text, delimiter) so panel re-renders for
  // unrelated reasons (resize, drawer, sibling state) don't re-parse the
  // whole file. AntD's <Table> handles its own internal memoization.
  const { columns, dataSource, truncated } = useMemo(() => {
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
    return { columns, dataSource, truncated }
  }, [text, delimiter])
  return (
    <div className="px-2">
      {truncated && (
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
        scroll={{ x: true, y: 'calc(100vh - 220px)' }}
        pagination={false}
      />
    </div>
  )
}
