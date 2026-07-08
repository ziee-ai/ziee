import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  DELIMITED_MAX_ROWS,
  XLSX_MAX_ROWS,
  capRows,
  parseDelimitedText,
} from './parse.ts'

function makeCsv(dataRows: number): string {
  const lines = ['id,name,value']
  for (let i = 1; i <= dataRows; i++) lines.push(`${i},row-${i},${i * 2}`)
  return lines.join('\n')
}

// ── TEST-4 (ITEM-5): CSV head-cap lifted from 10k; real truncation branch ────

test('parseDelimitedText returns ALL rows past the retired 10k cap (no truncation)', () => {
  const csv = makeCsv(15_000) // >10k, well below DELIMITED_MAX_ROWS
  const { headers, rows, truncated } = parseDelimitedText(csv, ',')
  assert.deepEqual(headers, ['id', 'name', 'value'])
  assert.equal(rows.length, 15_000)
  assert.equal(truncated, false)
  // A row that the old 10k head-cap would have dropped is present + parsed.
  assert.deepEqual(rows[14_999], ['15000', 'row-15000', '30000'])
})

test('parseDelimitedText really slices + sets truncated when the cap is exceeded', () => {
  // Exercise the REAL truncated:true branch by injecting a small cap (production
  // uses the 300k default) — this drives parseDelimitedText's own slice, not a
  // re-implemented predicate.
  const csv = makeCsv(50)
  const out = parseDelimitedText(csv, ',', 20)
  assert.equal(out.truncated, true)
  assert.equal(out.rows.length, 20)
  assert.deepEqual(out.rows[19], ['20', 'row-20', '40'])
  // Exactly at the cap → NOT truncated.
  const atCap = parseDelimitedText(makeCsv(20), ',', 20)
  assert.equal(atCap.truncated, false)
  assert.equal(atCap.rows.length, 20)
})

test('parseDelimitedText handles empty + header-only input', () => {
  assert.deepEqual(parseDelimitedText('', ','), { headers: [], rows: [], truncated: false })
  const headerOnly = parseDelimitedText('a,b,c', ',')
  assert.deepEqual(headerOnly.headers, ['a', 'b', 'c'])
  assert.equal(headerOnly.rows.length, 0)
  assert.equal(headerOnly.truncated, false)
})

test('DELIMITED_MAX_ROWS is the raised backstop (>10k)', () => {
  assert.ok(DELIMITED_MAX_ROWS >= 300_000)
  assert.ok(DELIMITED_MAX_ROWS > 10_000)
})

// ── TEST-5 (ITEM-6): the SHARED cap predicate used by the XLSX viewer ────────

test('capRows passes through below the cap and slices + flags above it', () => {
  const rows = Array.from({ length: 40 }, (_, i) => i)
  const below = capRows(rows, 100)
  assert.equal(below.truncated, false)
  assert.equal(below.rows.length, 40)

  const over = capRows(rows, 25)
  assert.equal(over.truncated, true)
  assert.equal(over.rows.length, 25)
  assert.equal(over.rows[24], 24)

  // Exactly at the cap → not truncated.
  const at = capRows(rows, 40)
  assert.equal(at.truncated, false)
  assert.equal(at.rows.length, 40)
})

test('XLSX_MAX_ROWS is the raised per-sheet backstop (>10k); the retired 10k no longer truncates', () => {
  assert.ok(XLSX_MAX_ROWS > 10_000)
  assert.ok(XLSX_MAX_ROWS >= 200_000)
  // XlsxBody feeds dataRows through capRows(_, XLSX_MAX_ROWS): a 10k-row sheet
  // (the retired cap) must NOT be truncated by the raised backstop.
  const tenK = Array.from({ length: 10_000 }, (_, i) => i)
  assert.equal(capRows(tenK, XLSX_MAX_ROWS).truncated, false)
})
