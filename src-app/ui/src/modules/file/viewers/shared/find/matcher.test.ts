import { test } from 'node:test'
import assert from 'node:assert/strict'
import { collectMatches } from './matcher.ts'

// ── TEST-3 (ITEM-4): find-in-document match semantics ────────────────────────

test('collectMatches is case-insensitive and counts every occurrence', () => {
  const m = collectMatches('The cat sat on the CAT mat', 'cat')
  assert.equal(m.length, 2)
  assert.deepEqual(m[0], { start: 4, end: 7 })
  assert.deepEqual(m[1], { start: 19, end: 22 })
})

test('collectMatches returns ascending, non-overlapping spans', () => {
  // 'aaaa' / 'aa' → 2 non-overlapping matches (not 3).
  const m = collectMatches('aaaa', 'aa')
  assert.deepEqual(m, [
    { start: 0, end: 2 },
    { start: 2, end: 4 },
  ])
})

test('an empty or whitespace-only needle matches nothing', () => {
  assert.deepEqual(collectMatches('anything', ''), [])
  assert.deepEqual(collectMatches('anything', '   '), [])
})

test('no match yields an empty array', () => {
  assert.deepEqual(collectMatches('hello world', 'zzz'), [])
})

test('an empty haystack yields an empty array', () => {
  assert.deepEqual(collectMatches('', 'x'), [])
})
