import { test, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import { getDraft, setDraft, clearDraft, NEW_DRAFT_KEY } from './chatDrafts.ts'

// Minimal in-memory localStorage stub (node:test has no DOM).
function installStorage() {
  const map = new Map<string, string>()
  ;(globalThis as any).localStorage = {
    getItem: (k: string) => (map.has(k) ? map.get(k)! : null),
    setItem: (k: string, v: string) => void map.set(k, v),
    removeItem: (k: string) => void map.delete(k),
    clear: () => map.clear(),
  }
  return map
}

beforeEach(() => {
  installStorage()
})

test('getDraft/setDraft roundtrip for a conversation key', () => {
  assert.equal(getDraft('conv-1'), '')
  setDraft('conv-1', 'hello world')
  assert.equal(getDraft('conv-1'), 'hello world')
})

test('setDraft with empty/whitespace text removes the key', () => {
  setDraft('conv-1', 'draft')
  assert.equal(getDraft('conv-1'), 'draft')
  setDraft('conv-1', '   ')
  assert.equal(getDraft('conv-1'), '')
})

test('drafts are isolated per key', () => {
  setDraft('conv-1', 'one')
  setDraft('conv-2', 'two')
  assert.equal(getDraft('conv-1'), 'one')
  assert.equal(getDraft('conv-2'), 'two')
})

test('clearDraft clears ONLY the given key (never the new bucket)', () => {
  setDraft(NEW_DRAFT_KEY, 'started in new chat')
  setDraft('conv-1', 'in conversation')
  clearDraft('conv-1')
  assert.equal(getDraft('conv-1'), '', 'the conversation draft is cleared')
  assert.equal(
    getDraft(NEW_DRAFT_KEY),
    'started in new chat',
    'a separate new-chat draft is NOT wiped by clearing a conversation draft',
  )
})

test('clearDraft on the new key clears only new', () => {
  setDraft(NEW_DRAFT_KEY, 'x')
  setDraft('conv-1', 'keep me')
  clearDraft(NEW_DRAFT_KEY)
  assert.equal(getDraft(NEW_DRAFT_KEY), '')
  assert.equal(getDraft('conv-1'), 'keep me')
})

test('storage failures degrade to no-op', () => {
  ;(globalThis as any).localStorage = {
    getItem: () => {
      throw new Error('blocked')
    },
    setItem: () => {
      throw new Error('blocked')
    },
    removeItem: () => {
      throw new Error('blocked')
    },
  }
  assert.equal(getDraft('conv-1'), '')
  assert.doesNotThrow(() => setDraft('conv-1', 'x'))
  assert.doesNotThrow(() => clearDraft('conv-1'))
})
