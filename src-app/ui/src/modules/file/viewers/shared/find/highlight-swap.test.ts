import { test } from 'node:test'
import assert from 'node:assert/strict'
import { escapeHtml, plainLineCodeInner } from '../chunking.ts'

// ── TEST-6 (ITEM-3): a chunk's plain→highlight swap preserves the exact text ──
//
// find-in-document builds Ranges by walking DOM TEXT NODES. When a windowed
// chunk upgrades from plain text to Shiki token spans, the *visible* (and thus
// searchable) text MUST be byte-identical, or find counts/ranges would shift as
// the user scrolls and chunks highlight. Both renderings differ only in the
// TAGS wrapping the text — never the text itself. These are DOM-free property
// tests of that invariant (the suite runs under node:test, no jsdom): a minimal
// tag-stripper + entity-decoder stands in for the browser's textContent.

/** Decode the exact entities `escapeHtml` produces + strip tags — a stand-in for
 *  `Element.textContent` over our generated HTML. `&amp;` is decoded LAST so a
 *  literal `&lt;` in the source (escaped to `&amp;lt;`) round-trips correctly. */
function htmlToText(html: string): string {
  return html
    .replace(/<[^>]*>/g, '')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
}

const TRICKY = [
  'plain ascii line',
  'tags <div> & "quotes" & <script>alert(1)</script>',
  'a & b < c > d',
  'unicode: café — naïve — 日本語 — 🎉',
  'already-escaped-looking: &lt; &amp; &gt;',
  '',
  '   leading + trailing spaces   ',
  '\tindented\twith\ttabs',
]

test('escapeHtml round-trips back to the original text', () => {
  for (const s of TRICKY) {
    assert.equal(htmlToText(escapeHtml(s)), s, `round-trip failed for: ${JSON.stringify(s)}`)
  }
})

test('plainLineCodeInner text equals the source line', () => {
  for (const line of TRICKY) {
    assert.equal(htmlToText(plainLineCodeInner(line)), line)
  }
})

test('a token-span (highlighted) rendering of the same line has identical text', () => {
  // Simulate Shiki wrapping each whitespace-delimited token in a colored span.
  // The concatenated textContent must still equal the source line — this is the
  // property that keeps find Ranges valid across the plain→highlight swap.
  for (const line of TRICKY) {
    const tokenized = line
      .split(/(\s+)/) // keep the whitespace tokens so the text reconstructs exactly
      .map(tok => (tok.trim() === '' ? escapeHtml(tok) : `<span style="color:#abc">${escapeHtml(tok)}</span>`))
      .join('')
    assert.equal(
      htmlToText(tokenized),
      htmlToText(plainLineCodeInner(line)),
      `plain vs tokenized text diverged for: ${JSON.stringify(line)}`,
    )
  }
})
