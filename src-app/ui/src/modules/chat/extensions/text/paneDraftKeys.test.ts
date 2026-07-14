import { test } from 'node:test'
import assert from 'node:assert/strict'
import { paneKeyOf, PaneDraftKeys } from './paneDraftKeys.ts'

// TEST-99 (audit #4): the per-pane capture that replaces the module-global draft
// key so two concurrent sends can't clobber each other's captured key.

test('paneKeyOf maps null/undefined pane to the single-pane slot', () => {
  assert.equal(paneKeyOf(undefined), '')
  assert.equal(paneKeyOf(null), '')
  assert.equal(paneKeyOf('pane-7'), 'pane-7')
})

test('two concurrent sends do NOT clobber each other (the bug)', () => {
  const d = new PaneDraftKeys()
  // Interleaved beforeSendMessage captures: pane B, then pane A (the module-global
  // `let` would leave only A's key, so B's onMessageSent read A's key).
  d.set('paneB', 'draft:convB')
  d.set('paneA', 'draft:convA')
  // Each pane's onMessageSent reads back ITS OWN key.
  assert.equal(d.take('paneB'), 'draft:convB')
  assert.equal(d.take('paneA'), 'draft:convA')
})

test('take is one-shot (read-and-clear)', () => {
  const d = new PaneDraftKeys()
  d.set('paneB', 'k')
  assert.equal(d.take('paneB'), 'k')
  assert.equal(d.take('paneB'), undefined, 'second take is cleared')
})

test('single-pane (null pane) is its own independent slot', () => {
  const d = new PaneDraftKeys()
  d.set(null, 'draft:single')
  d.set('paneB', 'draft:B')
  assert.equal(d.take(null), 'draft:single')
  assert.equal(d.take('paneB'), 'draft:B')
})
