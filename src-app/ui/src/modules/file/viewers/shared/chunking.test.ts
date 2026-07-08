import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  RAWCODE_CHUNK_LINES,
  RAWCODE_MAX_LINES,
  LINE_PX,
  LINE_PX_WRAP,
  chunkLines,
  chunkLineArray,
  applyLineCap,
  chunkReservedHeight,
} from './chunking.ts'

// ── TEST-1 (ITEM-1): chunkLines splits into contiguous ordered chunks ────────

test('chunkLines splits into contiguous chunks preserving order', () => {
  const text = Array.from({ length: 1250 }, (_, i) => `line ${i}`).join('\n')
  const chunks = chunkLines(text, 500)
  assert.equal(chunks.length, 3) // 500 + 500 + 250
  assert.equal(chunks[0].lines.length, 500)
  assert.equal(chunks[1].lines.length, 500)
  assert.equal(chunks[2].lines.length, 250)
  // order preserved
  assert.equal(chunks[0].lines[0], 'line 0')
  assert.equal(chunks[2].lines[249], 'line 1249')
})

test('chunkLines join round-trips byte-exactly to the source', () => {
  const text = ['a', 'b,c', '', 'd"e', 'f'].join('\n')
  const chunks = chunkLines(text, 2)
  assert.equal(chunks.map(c => c.text).join('\n'), text)
})

test('chunkLines carries the correct global startLine offset per chunk', () => {
  const text = Array.from({ length: 1100 }, (_, i) => String(i)).join('\n')
  const chunks = chunkLines(text, 500)
  assert.deepEqual(
    chunks.map(c => c.startLine),
    [0, 500, 1000],
  )
  // The global 1-based line number of a chunk's first line = startLine + 1.
  assert.equal(chunks[1].startLine + 1, 501)
})

test('chunkLines yields one chunk for empty and single-line input', () => {
  const empty = chunkLines('', 500)
  assert.equal(empty.length, 1)
  assert.deepEqual(empty[0].lines, [''])
  assert.equal(empty[0].startLine, 0)

  const one = chunkLines('only one', 500)
  assert.equal(one.length, 1)
  assert.deepEqual(one[0].lines, ['only one'])
})

test('chunkLineArray clamps a non-positive size to 1', () => {
  const chunks = chunkLineArray(['a', 'b', 'c'], 0)
  assert.equal(chunks.length, 3)
  assert.deepEqual(chunks.map(c => c.startLine), [0, 1, 2])
})

// ── TEST-2 (ITEM-2): applyLineCap lifts truncation to the raised backstop ────

test('applyLineCap passes through below the cap (truncated:false)', () => {
  const lines = Array.from({ length: 50 }, (_, i) => String(i))
  const out = applyLineCap(lines, 100)
  assert.equal(out.truncated, false)
  assert.equal(out.lines.length, 50)
})

test('applyLineCap slices to exactly the cap at/above it (truncated:true)', () => {
  const lines = Array.from({ length: 120 }, (_, i) => String(i))
  const out = applyLineCap(lines, 100)
  assert.equal(out.truncated, true)
  assert.equal(out.lines.length, 100)
  assert.equal(out.lines[99], '99')
})

test('RAWCODE_MAX_LINES is lifted far above the retired 10k cap', () => {
  // Regression guard on the lifted cap: a 25k-line file must NOT truncate.
  assert.ok(RAWCODE_MAX_LINES >= 300_000)
  assert.ok(RAWCODE_MAX_LINES > 10_000)
  const lines = Array.from({ length: 25_000 }, (_, i) => String(i))
  assert.equal(applyLineCap(lines, RAWCODE_MAX_LINES).truncated, false)
})

test('RAWCODE_CHUNK_LINES is a sane windowing size', () => {
  assert.ok(RAWCODE_CHUNK_LINES >= 100 && RAWCODE_CHUNK_LINES <= 2000)
})

// ── TEST-3 (ITEM-4): reserved height is wrap-aware + linear ───────────────────

test('chunkReservedHeight reserves more in wrap mode than no-wrap', () => {
  assert.ok(chunkReservedHeight(500, true) > chunkReservedHeight(500, false))
  assert.equal(chunkReservedHeight(500, false), 500 * LINE_PX)
  assert.equal(chunkReservedHeight(500, true), 500 * LINE_PX_WRAP)
})

test('chunkReservedHeight scales linearly with line count', () => {
  assert.equal(chunkReservedHeight(200, false), 2 * chunkReservedHeight(100, false))
  assert.equal(chunkReservedHeight(0, false), 0)
})
