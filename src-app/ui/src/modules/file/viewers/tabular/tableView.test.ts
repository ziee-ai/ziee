import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  type ExportColumn,
  type TabularRecord,
  exportFilename,
  rowsToAoa,
  rowsToDelimited,
} from './tableView.ts'

// Data columns as the viewer builds them (colKey → header title); the `#` gutter
// (`__rn`) is deliberately NOT in this list — the viewer excludes it (TEST-11).
const columns: ExportColumn[] = [
  { key: '0', title: 'Name' },
  { key: '1', title: 'Qty' },
]

const rows: TabularRecord[] = [
  { key: '0', __rn: '1', '0': 'Banana', '1': '10' },
  { key: '1', __rn: '2', '0': 'apple, green', '1': '2' },
]

// TEST-10 — CSV/TSV serialisation: delimiter, RFC-4180 quoting, order + exclusion
test('rowsToDelimited writes a header row and honours the delimiter', () => {
  const csv = rowsToDelimited(rows, columns, ',')
  assert.equal(csv, 'Name,Qty\r\nBanana,10\r\n"apple, green",2') // comma triggers quoting
  const tsv = rowsToDelimited(rows, columns, '\t')
  assert.equal(tsv, 'Name\tQty\r\nBanana\t10\r\napple, green\t2') // no tab → no quote
})

test('rowsToDelimited RFC-4180-quotes embedded quotes and newlines', () => {
  const r: TabularRecord[] = [{ key: '0', '0': 'a"b', '1': 'x\ny' }]
  const csv = rowsToDelimited(r, columns, ',')
  assert.equal(csv, 'Name,Qty\r\n"a""b","x\ny"')
})

test('rowsToDelimited emits ONLY the passed columns in order (exclusion + reorder)', () => {
  // Pass qty first, exclude name entirely → proves hidden-column exclusion.
  const only: ExportColumn[] = [{ key: '1', title: 'Qty' }]
  const csv = rowsToDelimited(rows, only, ',')
  assert.equal(csv, 'Qty\r\n10\r\n2')
})

// TEST-11 — the `#` gutter (__rn) is never an export column; row identity kept
test('the row-number gutter is excluded from exported data columns', () => {
  const csv = rowsToDelimited(rows, columns, ',')
  assert.ok(!csv.split('\r\n')[0].includes('#'))
  assert.ok(!csv.includes('\t1\t')) // __rn value not injected as a column
  // aoa mirrors the same column set (header + 2 data rows, 2 cells each)
  const aoa = rowsToAoa(rows, columns)
  assert.deepEqual(aoa[0], ['Name', 'Qty'])
  assert.equal(aoa.length, 3)
  assert.equal(aoa[1].length, 2)
})

test('exportFilename swaps the extension and appends -view', () => {
  assert.equal(exportFilename('data.csv', 'csv'), 'data-view.csv')
  assert.equal(exportFilename('sheet.xlsx', 'xlsx'), 'sheet-view.xlsx')
  assert.equal(exportFilename(undefined, 'csv'), 'export-view.csv')
})
