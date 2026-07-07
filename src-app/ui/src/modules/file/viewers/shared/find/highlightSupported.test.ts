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

test('isHighlightSupported is true when CSS.highlights + Highlight exist', () => {
  // Positively exercise the supported branch (node has neither global by default)
  // by stubbing the two the detector checks, then restoring.
  const g = globalThis as Record<string, unknown>
  const hadCSS = 'CSS' in g
  const hadHighlight = 'Highlight' in g
  const prevCSS = g.CSS
  const prevHighlight = g.Highlight
  try {
    g.CSS = { highlights: new Map() }
    g.Highlight = function Highlight() {} as unknown
    assert.equal(isHighlightSupported(), true)
    // Missing the constructor → false even with the registry present.
    g.Highlight = undefined
    assert.equal(isHighlightSupported(), false)
  } finally {
    if (hadCSS) g.CSS = prevCSS
    else delete g.CSS
    if (hadHighlight) g.Highlight = prevHighlight
    else delete g.Highlight
  }
})
