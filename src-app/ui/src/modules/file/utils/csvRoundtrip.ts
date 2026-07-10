import { parseDelimitedText } from '@/modules/file/viewers/tabular/parse'

/**
 * CSV ⟷ grid for the editable CSV canvas. Parsing reuses the tabular viewer's
 * RFC-4180 parser but with NO row cap (editing must never truncate → data-loss
 * on save). Serialization is edit-safe: RFC-4180 quoting, and — unlike the
 * viewer's EXPORT serializer — it does NOT neutralize formula-looking cells
 * (that's an export-safety transform; when editing we round-trip content
 * faithfully). Pure + unit-tested.
 */

export interface CsvGrid {
  headers: string[]
  rows: string[][]
}

const NO_CAP = Number.MAX_SAFE_INTEGER

function quoteCsvField(v: string): string {
  return /[",\n\r]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v
}

export function parseCsv(text: string): CsvGrid {
  const { headers, rows } = parseDelimitedText(text, ',', NO_CAP)
  return { headers, rows }
}

export function serializeCsv(grid: CsvGrid): string {
  const line = (cells: string[]) => cells.map(c => quoteCsvField(c ?? '')).join(',')
  return `${[line(grid.headers), ...grid.rows.map(line)].join('\n')}\n`
}

export function csvRoundtrip(text: string): string {
  return serializeCsv(parseCsv(text))
}
