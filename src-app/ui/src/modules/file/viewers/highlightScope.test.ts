import { test } from 'node:test'
import assert from 'node:assert/strict'
import { scopedHighlightKey } from './highlightScope.ts'

// TEST-73 (ITEM-49): the per-pane citation-highlight key. Two split panes opening
// a citation into the SAME document must hold INDEPENDENT highlights, and a null
// scope (single-pane / non-pane) must keep the bare fileId key (unchanged).

test('scopedHighlightKey: distinct panes on the same file → distinct keys', () => {
  const a = scopedHighlightKey('pane-A', 'file-1')
  const b = scopedHighlightKey('pane-B', 'file-1')
  assert.notEqual(a, b, 'same fileId in two panes must not collide')
  assert.equal(a, 'pane-A::file-1')
  assert.equal(b, 'pane-B::file-1')
})

test('scopedHighlightKey: null/undefined scope → the bare fileId (single-pane unchanged)', () => {
  assert.equal(scopedHighlightKey(null, 'file-1'), 'file-1')
  assert.equal(scopedHighlightKey(undefined, 'file-1'), 'file-1')
  assert.equal(scopedHighlightKey('', 'file-1'), 'file-1', 'empty scope → bare fileId')
})

test('scopedHighlightKey: same pane + same file is stable (setter and reader agree)', () => {
  assert.equal(
    scopedHighlightKey('pane-A', 'file-1'),
    scopedHighlightKey('pane-A', 'file-1'),
  )
})
