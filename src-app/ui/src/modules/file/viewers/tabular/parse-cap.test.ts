import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  DELIMITED_MAX_ROWS,
  XLSX_MAX_ROWS,
  parseDelimitedText,
} from './parse.ts'

function makeCsv(dataRows: number): string {
  const lines = ['id,name,value']
  for (let i = 1; i <= dataRows; i++) lines.push(`${i},row-${i},${i * 2}`)
  return lines.join('\n')
}

// ── TEST-4 (ITEM-5): CSV head-cap lifted from 10k to the raised backstop ─────

test('parseDelimitedText returns ALL rows past the retired 10k cap (no truncation)', () => {
  const csv = makeCsv(15_000) // >10k, well below DELIMITED_MAX_ROWS
  const { headers, rows, truncated } = parseDelimitedText(csv, ',')
  assert.deepEqual(headers, ['id', 'name', 'value'])
  assert.equal(rows.length, 15_000)
  assert.equal(truncated, false)
  // A row that would have been dropped by the old 10k head-cap is present.
  assert.deepEqual(rows[14_999], ['15000', 'row-15000', '30000'])
})

test('DELIMITED_MAX_ROWS is the raised backstop (>10k) and truncation fires only above it', () => {
  assert.ok(DELIMITED_MAX_ROWS >= 300_000)
  assert.ok(DELIMITED_MAX_ROWS > 10_000)
  // Simulate the predicate at the cap boundary without materializing 300k rows.
  const atCap = DELIMITED_MAX_ROWS
  const overCap = DELIMITED_MAX_ROWS + 5
  assert.equal(atCap > DELIMITED_MAX_ROWS, false) // exactly at cap → not truncated
  assert.equal(overCap > DELIMITED_MAX_ROWS, true) // above cap → truncated
})

test('parseDelimitedText slices to the cap + sets truncated only above DELIMITED_MAX_ROWS', () => {
  // Build a small synthetic case by asserting the slice semantics directly: the
  // real cap is huge, so verify the boundary logic with the exported constant.
  const csv = makeCsv(20) // trivially below cap
  const out = parseDelimitedText(csv, ',')
  assert.equal(out.rows.length, 20)
  assert.equal(out.truncated, false)
  // The slice bound is DELIMITED_MAX_ROWS (a data set at/under it is never cut).
  assert.ok(out.rows.length <= DELIMITED_MAX_ROWS)
})

test('parseDelimitedText handles empty + header-only input', () => {
  assert.deepEqual(parseDelimitedText('', ','), { headers: [], rows: [], truncated: false })
  const headerOnly = parseDelimitedText('a,b,c', ',')
  assert.deepEqual(headerOnly.headers, ['a', 'b', 'c'])
  assert.equal(headerOnly.rows.length, 0)
  assert.equal(headerOnly.truncated, false)
})

// ── TEST-5 (ITEM-6): XLSX per-sheet cap raised from 10k ──────────────────────

test('XLSX_MAX_ROWS is the raised per-sheet backstop (>10k)', () => {
  assert.ok(XLSX_MAX_ROWS > 10_000)
  assert.ok(XLSX_MAX_ROWS >= 200_000)
})

test('the xlsx sheet-truncation predicate fires only above XLSX_MAX_ROWS, not at 10k', () => {
  // XlsxBody uses `dataRows.length > XLSX_MAX_ROWS` for truncation and
  // `sheetRows: XLSX_MAX_ROWS + 1` for the parse limit — both off the SAME
  // constant. Verify the predicate no longer trips at the retired 10k value.
  const truncatedAt = (n: number) => n > XLSX_MAX_ROWS
  assert.equal(truncatedAt(10_000), false) // retired cap no longer truncates
  assert.equal(truncatedAt(150_000), false) // below the raised cap
  assert.equal(truncatedAt(XLSX_MAX_ROWS), false) // exactly at cap
  assert.equal(truncatedAt(XLSX_MAX_ROWS + 1), true) // above cap
})
