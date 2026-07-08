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

/**
 * Apply a row OOM-backstop cap. Below the cap the rows pass through
 * (`truncated:false`); above it they are sliced to exactly `cap`
 * (`truncated:true`). Single source of the truncation predicate for BOTH the
 * CSV/TSV (`parseDelimitedText`) and the XLSX (`XlsxBody`) viewers, so the
 * lifted-cap behaviour is exercised by real production code in the unit tests.
 */
export function capRows<T>(rows: T[], cap: number): { rows: T[]; truncated: boolean } {
  if (rows.length > cap) return { rows: rows.slice(0, cap), truncated: true }
  return { rows, truncated: false }
}

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
  cap: number = DELIMITED_MAX_ROWS,
): { headers: string[]; rows: string[][]; truncated: boolean } {
  const lines = text.split('\n').filter(l => l.trim() !== '')
  if (lines.length === 0) return { headers: [], rows: [], truncated: false }
  const headers = parseDelimitedLine(lines[0], delimiter)
  // Cap the data lines FIRST (via the shared predicate), then parse only the
  // kept lines — so a huge file never materializes fields past the cap. `cap` is
  // injectable purely so the unit tests can exercise the real truncated:true
  // branch without building 300k rows (production callers use the default).
  const { rows: keptLines, truncated } = capRows(lines.slice(1), cap)
  const rows = keptLines.map(l => parseDelimitedLine(l, delimiter))
  return { headers, rows, truncated }
}
