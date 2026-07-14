import { test } from 'node:test'
import assert from 'node:assert/strict'
import { toolCallInPane, pendingApprovalIdsInPane } from './toolCallPaneScope.ts'

// TEST-72 (ITEM-48): the process-global McpComposer.toolCalls map is scoped to a
// pane by whether the tool-call's carrying message (`message_id`) is one of the
// pane's own messages — so a pending approval in pane B is NOT treated as pane A's
// new approval (only the originating pane scrolls).

// Pane A holds messages m1,m2; pane B holds m3.
const paneA = new Set(['m1', 'm2'])
const paneB = new Set(['m3'])

test('toolCallInPane: a call belongs to the pane iff its message is in the pane', () => {
  assert.equal(toolCallInPane({ message_id: 'm1' }, paneA), true)
  assert.equal(toolCallInPane({ message_id: 'm3' }, paneA), false, 'B\'s message not in A')
  assert.equal(toolCallInPane({ message_id: 'm3' }, paneB), true)
  assert.equal(toolCallInPane({ message_id: undefined }, paneA), false, 'no message_id → not scoped')
  assert.equal(toolCallInPane({ message_id: null }, paneA), false)
})

test('pendingApprovalIdsInPane: only THIS pane\'s pending approvals (the wrong-pane scroll bug)', () => {
  const toolCalls = new Map<string, { status: string; message_id?: string }>([
    ['t1', { status: 'pending_approval', message_id: 'm1' }], // pane A
    ['t2', { status: 'pending_approval', message_id: 'm3' }], // pane B
    ['t3', { status: 'completed', message_id: 'm2' }], // pane A but not pending
    ['t4', { status: 'started', message_id: 'm1' }], // pane A but not pending
  ])
  assert.deepEqual(
    pendingApprovalIdsInPane(toolCalls, paneA),
    ['t1'],
    'pane A sees only its own pending approval, never pane B\'s',
  )
  assert.deepEqual(
    pendingApprovalIdsInPane(toolCalls, paneB),
    ['t2'],
    'pane B sees only its own',
  )
})

test('pendingApprovalIdsInPane: a pending approval in another conversation is ignored', () => {
  // A leftover pending approval from a previously-viewed conversation (message not
  // in either open pane) must scroll NEITHER pane.
  const toolCalls = new Map<string, { status: string; message_id?: string }>([
    ['stale', { status: 'pending_approval', message_id: 'gone-999' }],
  ])
  assert.deepEqual(pendingApprovalIdsInPane(toolCalls, paneA), [])
  assert.deepEqual(pendingApprovalIdsInPane(toolCalls, paneB), [])
})
