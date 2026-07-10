import { test, beforeEach } from 'node:test'
import assert from 'node:assert/strict'
import {
  workspaceStorageKey,
  loadWorkspace,
  saveWorkspace,
  clearWorkspace,
  pruneWorkspace,
  migrateV1toV2,
  type PersistedWorkspace,
} from './splitWorkspace.persist.ts'
import type { Pane } from './SplitView.store'

// TEST-48 (split-chat ITEM-26): per-user workspace persistence — save/load
// round-trip under `ziee-split-workspace-v2:<userId>`, hydrate-time prune (drop
// inaccessible + empty panes, collapse <2 to single-pane), and the v1→v2
// one-time migration. Runs against an in-memory `localStorage` shim (the browser
// boundary the module targets — the storage is mocked, the behaviour is real).

class MemStorage {
  private m = new Map<string, string>()
  getItem(k: string): string | null {
    return this.m.has(k) ? (this.m.get(k) as string) : null
  }
  setItem(k: string, v: string): void {
    this.m.set(k, String(v))
  }
  removeItem(k: string): void {
    this.m.delete(k)
  }
  clear(): void {
    this.m.clear()
  }
  keys(): string[] {
    return [...this.m.keys()]
  }
}

let mem: MemStorage
beforeEach(() => {
  mem = new MemStorage()
  ;(globalThis as { localStorage?: unknown }).localStorage = mem
})

const pane = (paneId: string, conversationId: string | null): Pane => ({
  paneId,
  conversationId,
  projectId: null,
})

const ws = (panes: Pane[], focusedPaneId: string | null): PersistedWorkspace => ({
  panes,
  focusedPaneId,
  dividerWidths: [],
  direction: 'vertical',
  mode: 'split',
})

test('key is namespaced per user', () => {
  assert.equal(workspaceStorageKey('u1'), 'ziee-split-workspace-v2:u1')
  assert.notEqual(workspaceStorageKey('u1'), workspaceStorageKey('u2'))
  assert.equal(workspaceStorageKey(null), 'ziee-split-workspace-v2:anon')
})

test('save→load round-trips a 2-pane split under the per-user key', () => {
  const w = ws([pane('p1', 'a'), pane('p2', 'b')], 'p2')
  saveWorkspace('u1', w)
  const loaded = loadWorkspace('u1')
  assert.deepEqual(loaded?.panes.map((p) => p.conversationId), ['a', 'b'])
  assert.equal(loaded?.focusedPaneId, 'p2')
})

test('a different user cannot read another user’s workspace', () => {
  saveWorkspace('u1', ws([pane('p1', 'a'), pane('p2', 'b')], 'p1'))
  assert.equal(loadWorkspace('u2'), null, 'user 2 sees nothing under their own key')
})

test('saving a collapsed (<2 pane) workspace REMOVES the blob', () => {
  saveWorkspace('u1', ws([pane('p1', 'a'), pane('p2', 'b')], 'p1'))
  assert.ok(loadWorkspace('u1'), 'precondition: a split is stored')
  saveWorkspace('u1', ws([pane('p1', 'a')], 'p1')) // collapsed to 1 pane
  assert.equal(loadWorkspace('u1'), null, 'a single-pane workspace is not persisted')
})

test('clearWorkspace removes the blob', () => {
  saveWorkspace('u1', ws([pane('p1', 'a'), pane('p2', 'b')], 'p1'))
  clearWorkspace('u1')
  assert.equal(loadWorkspace('u1'), null)
})

test('pruneWorkspace drops inaccessible panes + collapses when <2 survive', () => {
  const accessible = new Set(['a'])
  const pruned = pruneWorkspace(
    ws([pane('p1', 'a'), pane('p2', 'deleted')], 'p2'),
    (id) => accessible.has(id),
  )
  // Only 'a' survives → <2 → collapse to single-pane (URL-driven).
  assert.deepEqual(pruned.panes, [])
  assert.equal(pruned.focusedPaneId, null)
})

test('pruneWorkspace keeps a valid split, drops empty picker panes, re-homes focus', () => {
  const accessible = new Set(['a', 'b'])
  const pruned = pruneWorkspace(
    ws([pane('p1', 'a'), pane('p2', null), pane('p3', 'b')], 'p2'),
    (id) => accessible.has(id),
  )
  assert.deepEqual(
    pruned.panes.map((p) => p.conversationId),
    ['a', 'b'],
    'the empty picker pane is dropped; the two real conversations survive',
  )
  assert.equal(
    pruned.focusedPaneId,
    'p1',
    'focus was on the dropped empty pane → re-homed to the first survivor',
  )
})

test('migrateV1toV2 reads the old key exactly once, writes v2, clears v1', () => {
  // store-kit persist wraps the payload as { state, version }.
  mem.setItem(
    'ziee-split-view-v1',
    JSON.stringify({
      state: {
        panes: [pane('p1', 'a'), pane('p2', 'b')],
        focusedPaneId: 'p1',
        dividerWidths: [],
        direction: 'vertical',
        mode: 'split',
      },
      version: 0,
    }),
  )
  const migrated = migrateV1toV2('u1')
  assert.deepEqual(migrated?.panes.map((p) => p.conversationId), ['a', 'b'])
  assert.equal(mem.getItem('ziee-split-view-v1'), null, 'v1 key is cleared')
  assert.ok(loadWorkspace('u1'), 'the workspace now lives under the v2 per-user key')
  // A second call finds nothing to migrate (idempotent).
  assert.equal(migrateV1toV2('u1'), null, 'migration runs exactly once')
})

test('migrateV1toV2 with no v1 key returns null', () => {
  assert.equal(migrateV1toV2('u1'), null)
})
