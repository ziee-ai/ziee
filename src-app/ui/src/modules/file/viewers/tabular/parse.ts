/**
 * Pure delimited-text parsing + the per-viewer row-cap constants for the tabular
 * viewers. Extracted from `DelimitedTable.tsx` / `XlsxBody.tsx` so the parsing +
 * cap behaviour is unit-testable without pulling the React/kit-Table render tree
 * (the suite runs under `node:test`).
 */

/** Raised OOM-backstop row cap for CSV/TSV (was 10k). The FULL dataset is parsed
 *  and the kit Table virtualizes rendering (only visible rows mount), so
 *  sort/filter span the WHOLE file rather than a 10k head. The cap bounds ONLY
 *  the pathological narrow-row case (a 10 MB CSV of `a\n` rows is millions of
 *  records): the upstream 10 MB byte cap bounds bytes, not row COUNT. Real
 *  (wide-row) files sit far below it and never truncate. */
export const DELIMITED_MAX_ROWS = 300_000

/** Raised per-sheet OOM-backstop row cap for XLSX (was 10k). Lower than
 *  DELIMITED_MAX_ROWS because xlsx is zip-COMPRESSED — a 10 MB xlsx can
 *  decompress to far more rows than its byte size implies (the byte cap can't
 *  bound it at all) and each row is heavier (per-cell object). Applied to BOTH
 *  the `XLSX.read({ sheetRows })` parse limit and the post-parse slice. */
export const XLSX_MAX_ROWS = 200_000

/** Split one delimited line into fields, honouring RFC-4180 double-quoting
 *  (quoted fields may contain the delimiter; `""` is an escaped quote). */
export function parseDelimitedLine(line: string, delimiter: string): string[] {
  const fields: string[] = []
  let field = ''
  let inQuotes = false

  for (let i = 0; i < line.length; i++) {
    const ch = line[i]
    if (ch === '"') {
      if (inQuotes && line[i + 1] === '"') {
        field += '"'
        i++
      } else inQuotes = !inQuotes
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

/**
 * Parse delimited text into a header row + data rows. The FULL dataset is
 * returned unless it exceeds `DELIMITED_MAX_ROWS`, in which case it is sliced to
 * the cap and `truncated` is set (the 10k head-cap is retired — sort/filter now
 * operate over every returned row).
 */
export function parseDelimitedText(
  text: string,
  delimiter: string,
): { headers: string[]; rows: string[][]; truncated: boolean } {
  const lines = text.split('\n').filter(l => l.trim() !== '')
  if (lines.length === 0) return { headers: [], rows: [], truncated: false }
  const headers = parseDelimitedLine(lines[0], delimiter)
  const dataLines = lines.slice(1)
  const truncated = dataLines.length > DELIMITED_MAX_ROWS
  const rows = dataLines
    .slice(0, DELIMITED_MAX_ROWS)
    .map(l => parseDelimitedLine(l, delimiter))
  return { headers, rows, truncated }
}
