import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  taskItemLabel,
  taskListCounts,
  taskItemsFromFrame,
  subAgentChildToolStatus,
  subAgentRollupStatus,
  subAgentActivityFromChildren,
  type TaskItemVM,
  type SubAgentChildVM,
} from './agentActivity.ts'

const item = (over: Partial<TaskItemVM>): TaskItemVM => ({
  id: 'i',
  content: 'Run tests',
  active_form: 'Running tests',
  status: 'pending',
  ...over,
})

// CC's dual-form render rule (ITEM-36): the in_progress item shows its
// present-continuous active_form; every other item shows its imperative content.
test('in_progress item renders active_form; others render content', () => {
  assert.equal(taskItemLabel(item({ status: 'in_progress' })), 'Running tests')
  assert.equal(taskItemLabel(item({ status: 'pending' })), 'Run tests')
  assert.equal(taskItemLabel(item({ status: 'completed' })), 'Run tests')
})

test('label falls back across the two forms so a row is never blank', () => {
  assert.equal(taskItemLabel(item({ status: 'in_progress', active_form: '' })), 'Run tests')
  assert.equal(taskItemLabel(item({ status: 'pending', content: '' })), 'Running tests')
})

test('taskListCounts tallies per-status + total', () => {
  const c = taskListCounts([
    item({ status: 'completed' }),
    item({ status: 'completed' }),
    item({ status: 'in_progress' }),
    item({ status: 'pending' }),
  ])
  assert.deepEqual(c, { completed: 2, inProgress: 1, pending: 1, total: 4 })
})

test('taskItemsFromFrame tolerates a missing/null items array', () => {
  assert.deepEqual(taskItemsFromFrame({}), [])
  assert.deepEqual(taskItemsFromFrame({ items: null }), [])
  assert.equal(taskItemsFromFrame({ items: [item({})] }).length, 1)
})

// Sub-agent child status maps onto the shared ToolStatusIcon vocabulary so a
// child row uses the identical icon set as every tool-call card.
test('subAgentChildToolStatus maps onto the shared tool-status keys', () => {
  assert.equal(subAgentChildToolStatus('running'), 'running')
  assert.equal(subAgentChildToolStatus('completed'), 'success')
  assert.equal(subAgentChildToolStatus('failed'), 'failed')
})

test('subAgentRollupStatus: any-failed dominates, then any-running, else success', () => {
  const c = (s: SubAgentChildVM['status']): SubAgentChildVM => ({ id: 'c', label: 'x', status: s })
  assert.equal(subAgentRollupStatus([]), 'running')
  assert.equal(subAgentRollupStatus([c('running'), c('completed')]), 'running')
  assert.equal(subAgentRollupStatus([c('completed'), c('completed')]), 'success')
  assert.equal(subAgentRollupStatus([c('failed'), c('running')]), 'failed')
  assert.equal(subAgentRollupStatus([c('completed'), c('failed')]), 'failed')
})

test('subAgentActivityFromChildren wraps the frame children into a VM', () => {
  const a = subAgentActivityFromChildren([{ id: 'c', label: 'x', status: 'running' }])
  assert.equal(a.children.length, 1)
  assert.equal(a.children[0].id, 'c')
})
