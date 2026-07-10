import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { Workflow } from '@/api-client/types'
import { chooseInputMode, selectDeclaredInputs } from './inputMode.ts'

// TEST-7 (ITEM-4) — the drawer's typed-vs-JSON branch predicate. Drives the REAL
// `parseWorkflowIr` off a workflow's `compiled_ir_json` (no mock of the parse).

const wf = (id: string, ir: unknown): Workflow =>
  ({ id, name: id, compiled_ir_json: ir }) as unknown as Workflow

const withInputs = wf('wf-typed', {
  inputs: [
    { name: 'topic', required: true },
    { name: 'since', required: false, default: '2024' },
  ],
  steps: [],
})
const noInputs = wf('wf-json', { inputs: [], steps: [] })
const noIr = wf('wf-bare', null)
const workflows = [withInputs, noInputs, noIr]

test('a workflow declaring inputs → typed mode with those inputs', () => {
  const inputs = selectDeclaredInputs(workflows, 'wf-typed')
  assert.equal(inputs.length, 2)
  assert.deepEqual(
    inputs.map(i => i.name),
    ['topic', 'since'],
  )
  assert.equal(chooseInputMode(inputs), 'typed')
})

test('a workflow with an empty inputs[] → JSON fallback mode', () => {
  const inputs = selectDeclaredInputs(workflows, 'wf-json')
  assert.equal(inputs.length, 0)
  assert.equal(chooseInputMode(inputs), 'json')
})

test('a workflow with no compiled IR → JSON fallback mode', () => {
  assert.equal(chooseInputMode(selectDeclaredInputs(workflows, 'wf-bare')), 'json')
})

test('no workflow selected (empty id / unknown id) → JSON fallback mode', () => {
  assert.deepEqual(selectDeclaredInputs(workflows, ''), [])
  assert.deepEqual(selectDeclaredInputs(workflows, 'does-not-exist'), [])
  assert.equal(chooseInputMode(selectDeclaredInputs(workflows, '')), 'json')
})
