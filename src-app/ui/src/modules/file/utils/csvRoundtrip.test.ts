import { test } from 'node:test'
import assert from 'node:assert/strict'
import { csvRoundtrip, parseCsv, serializeCsv } from './csvRoundtrip.ts'

test('parseCsv reads headers + rows', () => {
  const g = parseCsv('Name,Qty\nApple,3\nBanana,10\n')
  assert.deepEqual(g.headers, ['Name', 'Qty'])
  assert.equal(g.rows.length, 2)
  assert.deepEqual(g.rows[0], ['Apple', '3'])
})

test('round-trip preserves quoted fields with embedded commas', () => {
  const csv = 'Name,Note\n"Smith, John","a, b, c"\n'
  const out = csvRoundtrip(csv)
  const g = parseCsv(out)
  assert.deepEqual(g.rows[0], ['Smith, John', 'a, b, c'])
  assert.match(out, /"Smith, John"/)
})

test('round-trip preserves embedded quotes + newlines', () => {
  const g = { headers: ['a'], rows: [['he said "hi"'], ['line1\nline2']] }
  const out = serializeCsv(g)
  const back = parseCsv(out)
  assert.equal(back.rows[0][0], 'he said "hi"')
  // Embedded newline field round-trips (quoted).
  assert.match(out, /"line1\nline2"/)
})

test('editing a cell serializes to valid CSV', () => {
  const g = parseCsv('a,b\n1,2\n')
  g.rows[0][1] = 'edited'
  assert.equal(serializeCsv(g), 'a,b\n1,edited\n')
})

test('does NOT neutralize formula-looking cells (edit fidelity)', () => {
  const out = serializeCsv({ headers: ['f'], rows: [['=SUM(A1:A2)']] })
  assert.match(out, /=SUM\(A1:A2\)/)
})

test('empty input is safe', () => {
  assert.equal(typeof csvRoundtrip(''), 'string')
})
