import { test } from 'node:test'
import assert from 'node:assert/strict'
import { scopeFootnoteId, scopeHref, isFootnoteLabel } from './footnoteScope.ts'

// Guards the footnote-reference-click fix: Streamdown v2 double-prefixes
// footnote ids (`user-content-user-content-fn-N`) while leaving the ref href
// single-prefixed, so the scoping must be prefix-count-agnostic and produce the
// SAME scoped element for both the href and the definition id. Pure, no DOM.

const CID = 'c9'

// TEST-1: the double-prefix regression + single-prefix parity.
test('scopeFootnoteId: double-prefixed id (the bug) and single-prefix both scope to the same target', () => {
  assert.equal(scopeFootnoteId('user-content-user-content-fn-1', CID), 'c9-fn-1')
  assert.equal(scopeFootnoteId('user-content-fn-1', CID), 'c9-fn-1')
})

// TEST-2: zero prefix, custom label, fnref kind, multi-use suffix, passthrough.
test('scopeFootnoteId: zero-prefix / custom label / fnref / multi-use suffix', () => {
  assert.equal(scopeFootnoteId('fn-1', CID), 'c9-fn-1')
  assert.equal(scopeFootnoteId('user-content-fn-note', CID), 'c9-fn-note')
  assert.equal(scopeFootnoteId('user-content-user-content-fnref-1', CID), 'c9-fnref-1')
  // A footnote referenced twice: remark emits `fnref-1-2` for the 2nd use.
  assert.equal(scopeFootnoteId('user-content-fnref-1-2', CID), 'c9-fnref-1-2')
})

test('scopeFootnoteId: non-footnote ids and undefined pass through unchanged', () => {
  assert.equal(scopeFootnoteId('some-heading', CID), 'some-heading')
  assert.equal(scopeFootnoteId('user-content-footnote-label', CID), 'user-content-footnote-label')
  assert.equal(scopeFootnoteId(undefined, CID), undefined)
  assert.equal(scopeFootnoteId('', CID), '')
})

// TEST-3: href scoping — footnote hash (any prefix count), heading hash, external.
test('scopeHref: footnote hash (single + zero prefix) scopes to the same target the li id gets', () => {
  assert.equal(scopeHref('#user-content-fn-1', CID), '#c9-fn-1')
  assert.equal(scopeHref('#fn-1', CID), '#c9-fn-1')
  assert.equal(scopeHref('#user-content-user-content-fnref-1', CID), '#c9-fnref-1')
})

test('scopeHref: plain in-page hash retargets at this message\'s slugged heading', () => {
  assert.equal(scopeHref('#Some Section', CID), '#c9-h-some-section')
  // URL-encoded fragment is decoded before slugifying.
  assert.equal(scopeHref('#Some%20Section', CID), '#c9-h-some-section')
})

test('scopeHref: external URLs and undefined pass through unchanged', () => {
  assert.equal(scopeHref('https://example.test/x', CID), 'https://example.test/x')
  assert.equal(scopeHref('mailto:a@b.test', CID), 'mailto:a@b.test')
  assert.equal(scopeHref(undefined, CID), undefined)
})

// TEST-4: footnotes-label suppression predicate, prefix-agnostic.
test('isFootnoteLabel: true for footnote-label with any prefix count, false otherwise', () => {
  assert.equal(isFootnoteLabel('footnote-label'), true)
  assert.equal(isFootnoteLabel('user-content-footnote-label'), true)
  assert.equal(isFootnoteLabel('user-content-user-content-footnote-label'), true)
  assert.equal(isFootnoteLabel('user-content-fn-1'), false)
  assert.equal(isFootnoteLabel('some-heading'), false)
  assert.equal(isFootnoteLabel(undefined), false)
})
