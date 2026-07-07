import { test } from 'node:test'
import assert from 'node:assert/strict'
import { isHighlightSupported } from './highlightSupported.ts'

// ── TEST-4 (ITEM-5): feature-detect never throws + falls back off-DOM ─────────

test('isHighlightSupported returns a boolean and never throws', () => {
  const v = isHighlightSupported()
  assert.equal(typeof v, 'boolean')
})

test('isHighlightSupported is false in the non-DOM node env (no CSS.highlights)', () => {
  // node --test has no `CSS` global → the fallback (native find) path is taken,
  // proving the FindButton/Ctrl-F are correctly suppressed when the API is absent.
  assert.equal(isHighlightSupported(), false)
})
