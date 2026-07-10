import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  openConversationInWorkspace,
  type ReconcileIntent,
  type ReconcileOutcome,
  type WorkspaceLayout,
} from './reconcile.ts'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import type { Pane } from '@/modules/chat/core/stores/SplitView.store'

// TEST-47 (split-chat ITEM-25): the pure `openConversationInWorkspace` reducer —
// the SINGLE rule every entry point routes through. Table-driven, one case per
// branch. A deterministic paneId generator (`n1`, `n2`, …) is injected so the
// pure reducer is fully assertable.

const pane = (paneId: string, conversationId: string | null): Pane => ({
  paneId,
  conversationId,
  projectId: null,
})

/** A fresh deterministic id generator per case (`n1`, `n2`, …). */
const idGen = () => {
  let i = 0
  return () => `n${++i}`
}

interface Case {
  name: string
  layout: WorkspaceLayout
  currentConversationId: string | null
  conversationId: string
  intent: ReconcileIntent
  expect: (r: {
    next: WorkspaceLayout
    outcome: ReconcileOutcome
  }) => void
}

const cases: Case[] = [
  {
    name: "auto + conversation already in a pane → focus it (no duplicate)",
    layout: { panes: [pane('p1', 'a'), pane('p2', 'b')], focusedPaneId: 'p1' },
    currentConversationId: 'a',
    conversationId: 'b',
    intent: 'auto',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'focus')
      assert.equal((r.outcome as { paneId: string }).paneId, 'p2')
      assert.equal(r.next.focusedPaneId, 'p2')
      assert.equal(r.next.panes.length, 2, 'no pane added')
    },
  },
  {
    name: "newPane + conversation already open → focus it, still no duplicate",
    layout: { panes: [pane('p1', 'a'), pane('p2', 'b')], focusedPaneId: 'p1' },
    currentConversationId: 'a',
    conversationId: 'b',
    intent: 'newPane',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'focus')
      assert.equal(r.next.panes.length, 2, 'newPane must not duplicate an open conversation')
    },
  },
  {
    name: "newPane from single-pane → seeds [current | X], focuses X",
    layout: { panes: [], focusedPaneId: null },
    currentConversationId: 'a',
    conversationId: 'b',
    intent: 'newPane',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'addPane')
      assert.deepEqual(
        r.next.panes.map((p) => p.conversationId),
        ['a', 'b'],
        'pane 0 seeded from the URL conversation, X appended',
      )
      assert.equal(r.next.focusedPaneId, r.next.panes[1].paneId, 'the new pane is focused')
    },
  },
  {
    name: "newPane inserts AFTER the focused pane",
    layout: {
      panes: [pane('p1', 'a'), pane('p2', 'b')],
      focusedPaneId: 'p1',
    },
    currentConversationId: 'a',
    conversationId: 'c',
    intent: 'newPane',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'addPane')
      assert.deepEqual(
        r.next.panes.map((p) => p.conversationId),
        ['a', 'c', 'b'],
        'the new pane is spliced directly after the focused pane p1',
      )
    },
  },
  {
    name: "newPane at MAX_PANES → capReached, layout unchanged",
    layout: {
      panes: Array.from({ length: SPLIT_LIMITS.MAX_PANES }, (_, i) =>
        pane(`p${i}`, `c${i}`),
      ),
      focusedPaneId: 'p0',
    },
    currentConversationId: 'c0',
    conversationId: 'overflow',
    intent: 'newPane',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'capReached')
      assert.equal(r.next.panes.length, SPLIT_LIMITS.MAX_PANES, 'no pane added past the cap')
    },
  },
  {
    name: "auto while split → replaces the focused pane's conversation",
    layout: { panes: [pane('p1', 'a'), pane('p2', 'b')], focusedPaneId: 'p2' },
    currentConversationId: 'a',
    conversationId: 'c',
    intent: 'auto',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'replace')
      assert.equal((r.outcome as { paneId: string }).paneId, 'p2')
      assert.deepEqual(
        r.next.panes.map((p) => p.conversationId),
        ['a', 'c'],
        'only the focused pane p2 is repointed',
      )
    },
  },
  {
    name: "replaceFocused while split → replaces the focused pane",
    layout: { panes: [pane('p1', 'a'), pane('p2', 'b')], focusedPaneId: 'p1' },
    currentConversationId: 'a',
    conversationId: 'c',
    intent: 'replaceFocused',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'replace')
      assert.deepEqual(
        r.next.panes.map((p) => p.conversationId),
        ['c', 'b'],
      )
    },
  },
  {
    name: "auto with an empty workspace → navigate, state unchanged",
    layout: { panes: [], focusedPaneId: null },
    currentConversationId: null,
    conversationId: 'a',
    intent: 'auto',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'navigate')
      assert.equal((r.outcome as { conversationId: string }).conversationId, 'a')
      assert.equal(r.next.panes.length, 0, 'a plain navigate does not spawn a pane')
    },
  },
  {
    name: "auto with a lone pane → navigate AND keep the lone pane in sync",
    layout: { panes: [pane('p1', 'a')], focusedPaneId: 'p1' },
    currentConversationId: 'a',
    conversationId: 'b',
    intent: 'auto',
    expect: (r) => {
      assert.equal(r.outcome.kind, 'navigate')
      assert.deepEqual(
        r.next.panes.map((p) => p.conversationId),
        ['b'],
        'the single pane follows the navigate so it does not dangle on the old conversation',
      )
      assert.equal(r.next.panes[0].paneId, 'p1', 'the lone pane keeps its stable id')
    },
  },
]

for (const c of cases) {
  test(`reconcile: ${c.name}`, () => {
    const result = openConversationInWorkspace({
      layout: c.layout,
      currentConversationId: c.currentConversationId,
      conversationId: c.conversationId,
      projectId: null,
      intent: c.intent,
      newPaneId: idGen(),
    })
    c.expect(result)
  })
}

test('reconcile: purity — the input layout object is never mutated', () => {
  const layout: WorkspaceLayout = {
    panes: [pane('p1', 'a'), pane('p2', 'b')],
    focusedPaneId: 'p1',
  }
  const snapshot = JSON.stringify(layout)
  openConversationInWorkspace({
    layout,
    currentConversationId: 'a',
    conversationId: 'c',
    projectId: null,
    intent: 'newPane',
    newPaneId: idGen(),
  })
  assert.equal(JSON.stringify(layout), snapshot, 'reducer must not mutate its input')
})
