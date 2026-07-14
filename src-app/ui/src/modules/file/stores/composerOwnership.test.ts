import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  SINGLE_PANE_KEY,
  composerPaneKey,
  ownsId,
  ownedIds,
  snapshotOwned,
  mergeOwnedInto,
} from './composerOwnership.ts'

// ITEM-40 (split-chat): per-pane composer file ownership. A file attached in one
// split pane's composer must be visible/removable/backup-restorable ONLY in that
// pane — the isolation FB-6 flagged as untested. These pure helpers are what the
// File.store buffer actions delegate to, so proving them here proves the primitive.

const PANE_A = 'pane-A'
const PANE_B = 'pane-B'

test('composerPaneKey: a pane id maps to itself; null/empty → the single-pane key', () => {
  assert.equal(composerPaneKey(PANE_A), PANE_A)
  assert.equal(composerPaneKey(null), SINGLE_PANE_KEY)
  assert.equal(composerPaneKey(undefined), SINGLE_PANE_KEY)
  assert.equal(composerPaneKey(''), SINGLE_PANE_KEY)
})

test('ownsId: an owner-less entry answers to the single-pane key, not "unowned"', () => {
  const owner = new Map<string, string>([['f1', PANE_A]])
  assert.equal(ownsId(owner, 'f1', PANE_A), true)
  assert.equal(ownsId(owner, 'f1', PANE_B), false)
  // f2 has no owner entry → resolves to SINGLE_PANE_KEY.
  assert.equal(ownsId(owner, 'f2', SINGLE_PANE_KEY), true)
  assert.equal(ownsId(owner, 'f2', PANE_A), false)
})

test('ownedIds: filters a shared buffer to ONLY the queried pane (attach-in-B not visible to A)', () => {
  // Two panes attach files into the SAME shared buffer; owner map segregates them.
  const owner = new Map<string, string>([
    ['a1', PANE_A],
    ['b1', PANE_B],
    ['b2', PANE_B],
  ])
  const buffer = new Map<string, unknown>([
    ['a1', {}],
    ['b1', {}],
    ['b2', {}],
  ])
  assert.deepEqual(ownedIds(buffer.keys(), owner, PANE_B), ['b1', 'b2'])
  assert.deepEqual(
    ownedIds(buffer.keys(), owner, PANE_A),
    ['a1'],
    "A sees only its own file, never B's",
  )
})

test('ownedIds → clearFiles scoping: removing B leaves A untouched', () => {
  const owner = new Map<string, string>([
    ['a1', PANE_A],
    ['b1', PANE_B],
  ])
  const buffer = new Map<string, unknown>([
    ['a1', {}],
    ['b1', {}],
  ])
  // Emulate clearFiles(PANE_B): delete only B's owned ids.
  const removed = ownedIds(buffer.keys(), owner, PANE_B)
  for (const id of removed) {
    buffer.delete(id)
    owner.delete(id)
  }
  assert.deepEqual([...buffer.keys()], ['a1'], "A's attachment survives B's clear")
  assert.equal(owner.get('a1'), PANE_A)
})

test('snapshotOwned: a backup captures EXACTLY the sending pane\'s entries (empty when none)', () => {
  const owner = new Map<string, string>([
    ['a1', PANE_A],
    ['b1', PANE_B],
  ])
  const buffer = new Map<string, string>([
    ['a1', 'fileA'],
    ['b1', 'fileB'],
  ])
  const snapB = snapshotOwned(buffer, owner, PANE_B)
  assert.deepEqual([...snapB.entries()], [['b1', 'fileB']])
  // A pane with no owned entries backs up an empty slot (not the whole buffer).
  const snapEmpty = snapshotOwned(buffer, owner, 'pane-C')
  assert.equal(snapEmpty.size, 0)
  // The source buffer is never mutated.
  assert.equal(buffer.size, 2)
})

test('mergeOwnedInto: restoreFromBackup MERGES — never a wholesale replace of the other pane', () => {
  // B suffered a stream error and restores its backup while A is live-editing.
  const current = new Map<string, string>([['a1', 'fileA-live']])
  const owner = new Map<string, string>([['a1', PANE_A]])
  const backupB = new Map<string, string>([['b1', 'fileB-restored']])

  const { next, nextOwner } = mergeOwnedInto(current, owner, backupB, PANE_B)

  // A's live entry is untouched; B's is restored + owner-stamped to B.
  assert.deepEqual(
    [...next.entries()].sort(),
    [
      ['a1', 'fileA-live'],
      ['b1', 'fileB-restored'],
    ],
  )
  assert.equal(nextOwner.get('a1'), PANE_A, "A's ownership preserved")
  assert.equal(nextOwner.get('b1'), PANE_B, "restored entry stamped to B")
  // Inputs are not mutated (immer-safe): current still lacks b1.
  assert.equal(current.has('b1'), false)
  assert.equal(owner.has('b1'), false)
})

test('two panes keep independent backup slots (snapshot + restore round-trip)', () => {
  const owner = new Map<string, string>([
    ['a1', PANE_A],
    ['b1', PANE_B],
  ])
  const buffer = new Map<string, string>([
    ['a1', 'A'],
    ['b1', 'B'],
  ])
  const snapA = snapshotOwned(buffer, owner, PANE_A)
  const snapB = snapshotOwned(buffer, owner, PANE_B)

  // Clear BOTH panes from the buffer (both sent), then restore only A.
  const cleared = new Map<string, string>()
  const clearedOwner = new Map<string, string>()
  const restoredA = mergeOwnedInto(cleared, clearedOwner, snapA, PANE_A)

  assert.deepEqual([...restoredA.next.entries()], [['a1', 'A']])
  assert.equal(restoredA.next.has('b1'), false, "A's restore does not resurrect B")
  // B's slot is still intact and independent, ready for its own restore.
  assert.deepEqual([...snapB.entries()], [['b1', 'B']])
})
