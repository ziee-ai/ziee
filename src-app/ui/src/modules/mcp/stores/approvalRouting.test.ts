import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  PENDING_CONVERSATION_KEY,
  pendingConversationKey,
  approvalKeyOf,
  addApprovalDecisionTo,
  getApprovalDecisionsFrom,
  clearApprovalDecisionsIn,
} from './approvalRouting.ts'

// ITEM-33 (split-chat): the wrong-pane tool-approval fix. Approvals are keyed by
// the ORIGINATING conversation, so approving a tool in one split pane can NEVER
// be picked up by another pane's send — the security-adjacent guarantee.

const dec = (id: string) => ({ tool_use_id: id, decision: 'approve', note: 't' })

test('approvalKeyOf maps a conversation to its key; null → pending', () => {
  assert.equal(approvalKeyOf('conv-A'), 'conv-A')
  assert.equal(approvalKeyOf(null), PENDING_CONVERSATION_KEY)
  assert.equal(approvalKeyOf(undefined), PENDING_CONVERSATION_KEY)
})

test('an approval added for conversation A is visible ONLY to A, not B (wrong-pane bug)', () => {
  let m = new Map()
  m = addApprovalDecisionTo(m, 'conv-A', dec('tool-1'))
  assert.deepEqual(getApprovalDecisionsFrom(m, 'conv-A').map((d) => d.tool_use_id), ['tool-1'])
  assert.deepEqual(getApprovalDecisionsFrom(m, 'conv-B'), [], 'the OTHER pane sees nothing')
})

test('two panes accumulate approvals independently', () => {
  let m = new Map()
  m = addApprovalDecisionTo(m, 'conv-A', dec('a1'))
  m = addApprovalDecisionTo(m, 'conv-B', dec('b1'))
  m = addApprovalDecisionTo(m, 'conv-A', dec('a2'))
  assert.deepEqual(getApprovalDecisionsFrom(m, 'conv-A').map((d) => d.tool_use_id), ['a1', 'a2'])
  assert.deepEqual(getApprovalDecisionsFrom(m, 'conv-B').map((d) => d.tool_use_id), ['b1'])
})

test('clearing A after its send does NOT clear B', () => {
  let m = new Map()
  m = addApprovalDecisionTo(m, 'conv-A', dec('a1'))
  m = addApprovalDecisionTo(m, 'conv-B', dec('b1'))
  m = clearApprovalDecisionsIn(m, 'conv-A')
  assert.deepEqual(getApprovalDecisionsFrom(m, 'conv-A'), [])
  assert.deepEqual(getApprovalDecisionsFrom(m, 'conv-B').map((d) => d.tool_use_id), ['b1'])
})

test('the helpers never mutate the input map (immer-safe)', () => {
  const m0 = new Map()
  const m1 = addApprovalDecisionTo(m0, 'conv-A', dec('a1'))
  assert.equal(m0.size, 0, 'add did not mutate the original')
  const m2 = clearApprovalDecisionsIn(m1, 'conv-A')
  assert.equal(m1.get('conv-A')?.length, 1, 'clear did not mutate its input')
  assert.equal(m2.has('conv-A'), false)
})

// TEST-77 (ITEM-51): the per-PANE pending MCP config key — two split panes each
// composing a NEW chat must edit their OWN pending config, not a shared one.
test('pendingConversationKey: per-pane pending key; null pane → the bare key', () => {
  assert.equal(pendingConversationKey(null), PENDING_CONVERSATION_KEY)
  assert.equal(pendingConversationKey(undefined), PENDING_CONVERSATION_KEY)
  assert.equal(pendingConversationKey(''), PENDING_CONVERSATION_KEY, 'empty pane id → single-pane bare key')
  assert.equal(pendingConversationKey('pane-A'), `${PENDING_CONVERSATION_KEY}:pane-A`)
  assert.notEqual(
    pendingConversationKey('pane-A'),
    pendingConversationKey('pane-B'),
    'two new-chat panes get distinct pending config keys (no cross-pane leak)',
  )
})
