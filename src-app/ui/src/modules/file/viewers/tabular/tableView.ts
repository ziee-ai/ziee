// Export / copy helpers for the tabular file viewer. Kept dependency-free and
// erasable-TS so `node --test "src/**/*.test.ts"` can exercise the pure
// serialisers; the XLSX build + the DOM download run only in the browser.

/** A column to export: the record key to read + the header title to write. */
export interface ExportColumn {
  key: string
  title: string
}

/** Tabular row record — the DelimitedTable/XlsxBody shape (colKey → cell value,
 *  plus `__rn` gutter + `key`). Only the ExportColumn keys are read. */
export type TabularRecord = Record<string, string>

// True when a string parses as a plain finite number (so a legit negative like
// "-5" or "+3" is NOT treated as a formula below).
function isPlainNumber(v: string): boolean {
  const s = v.trim()
  return s !== '' && Number.isFinite(Number(s))
}

// CSV/formula-injection neutralization: a spreadsheet evaluates a cell that
// begins with = + - @ (or a leading tab/CR) as a formula. Since the exported
// data may come from an untrusted viewed file, prefix such a cell with a single
// quote so Excel/Sheets treat it as literal text — but leave real numbers alone.
function neutralizeFormula(v: string): string {
  if (v === '') return v
  if (/^[=+\-@\t\r]/.test(v) && !isPlainNumber(v)) return `'${v}`
  return v
}

// RFC-4180 field quoting: wrap in double quotes and double any embedded quote
// when the value contains the delimiter, a quote, or a newline/CR.
function quoteField(v: string, delimiter: string): string {
  if (v.includes(delimiter) || v.includes('"') || v.includes('\n') || v.includes('\r')) {
    return `"${v.replace(/"/g, '""')}"`
  }
  return v
}

/** Serialise rows to delimited text (CSV/TSV) with a header row, honouring the
 *  active delimiter, RFC-4180 quoting, and the caller-supplied column subset +
 *  order (so filtered/sorted views and hidden-column exclusion round-trip).
 *  Cells that would be interpreted as spreadsheet formulas are neutralized. */
export function rowsToDelimited(rows: TabularRecord[], columns: ExportColumn[], delimiter: string): string {
  const header = columns.map(c => quoteField(c.title, delimiter)).join(delimiter)
  const body = rows.map(r =>
    columns.map(c => quoteField(neutralizeFormula(r[c.key] ?? ''), delimiter)).join(delimiter),
  )
  return [header, ...body].join('\r\n')
}

/** Rows → a 2-D array-of-arrays (header + data) for xlsx `aoa_to_sheet`. */
export function rowsToAoa(rows: TabularRecord[], columns: ExportColumn[]): string[][] {
  return [columns.map(c => c.title), ...rows.map(r => columns.map(c => r[c.key] ?? ''))]
}

/** Build an .xlsx Blob from the current view (dynamic-imports `xlsx`, mirroring
 *  XlsxBody's parse-side import). */
export async function viewToXlsxBlob(
  rows: TabularRecord[],
  columns: ExportColumn[],
  sheetName = 'Sheet1',
): Promise<Blob> {
  const XLSX = await import('xlsx')
  const ws = XLSX.utils.aoa_to_sheet(rowsToAoa(rows, columns))
  const wb = XLSX.utils.book_new()
  // Excel sheet names are capped at 31 chars and disallow a few characters.
  XLSX.utils.book_append_sheet(wb, ws, sheetName.replace(/[\\/?*[\]:]/g, '_').slice(0, 31) || 'Sheet1')
  const out = XLSX.write(wb, { type: 'array', bookType: 'xlsx' }) as ArrayBuffer
  return new Blob([out], {
    type: 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet',
  })
}

/** Trigger a browser download of a Blob (object-URL + transient anchor). */
export function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = filename
  document.body.appendChild(a)
  a.click()
  a.remove()
  // Revoke on the next tick so the click's navigation has committed.
  setTimeout(() => URL.revokeObjectURL(url), 0)
}

/** Download delimited text as a file. */
export function downloadDelimited(text: string, filename: string, delimiter: string): void {
  const mime = delimiter === '\t' ? 'text/tab-separated-values' : 'text/csv'
  downloadBlob(new Blob([text], { type: `${mime};charset=utf-8` }), filename)
}

/** Derive an export filename from the original file name + a `-view` suffix. */
export function exportFilename(original: string | undefined, ext: string): string {
  const stem = (original ?? 'export').replace(/\.[^./\\]+$/, '')
  return `${stem || 'export'}-view.${ext}`
}

/** The tabular viewer's current view snapshot. The body publishes it into
 *  `FileStore.fileTabularView` (keyed by file id) so the file-viewer header's
 *  Export / Copy-selection actions can act on the CURRENT view — matching the
 *  filtered/sorted rows, visible-column subset, and cell selection the user
 *  sees. Absent entry ⇒ the body hasn't published yet (header actions disabled). */
export interface TabularViewState {
  /** Rows in the current (filtered/sorted) view order. */
  rows: TabularRecord[]
  /** Visible columns (key + title) in display order (honours the chooser). */
  columns: ExportColumn[]
  /** Active delimiter (',' for CSV, '\t' for TSV). */
  delimiter: string
  /** Original file name, for the `-view` export filename. */
  fileName?: string
  /** The current cell/row selection serialised as TSV ('' when none). */
  selectionTsv: string
}

/** Export the current view as a delimited file — only the filtered/sorted rows,
 *  honouring the visible-column subset. Formerly triggered by the body toolbar;
 *  now driven from the file-viewer header (chrome). */
export function exportTabularView(view: TabularViewState): void {
  const ext = view.delimiter === '\t' ? 'tsv' : 'csv'
  const text = rowsToDelimited(view.rows, view.columns, view.delimiter)
  downloadDelimited(text, exportFilename(view.fileName, ext), view.delimiter)
}

/** The text a Copy-selection writes: the current selection as TSV, or — when
 *  nothing is selected — the whole view as TSV (formula-neutralized via
 *  rowsToDelimited). Pure (the clipboard write lives in copyTabularSelection). */
export function tabularClipboardText(view: TabularViewState): string {
  return view.selectionTsv || rowsToDelimited(view.rows, view.columns, '\t')
}

/** Copy the current selection (or the whole view as a fallback) to the clipboard
 *  as TSV. Returns whether the write succeeded so the caller can toast. */
export async function copyTabularSelection(view: TabularViewState): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(tabularClipboardText(view))
    return true
  } catch {
    return false
  }
}
