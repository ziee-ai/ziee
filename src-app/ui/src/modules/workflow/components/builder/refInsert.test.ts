import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { InputDef } from '@/api-client/types'
import { enumerateRefs, stepOutputHint } from './refInsert.ts'
import type { BuilderStep } from './stepForms.ts'

// TEST-17 — the RefInsertMenu enumeration. For the step at index i it must list
// every workflow input plus ONLY the prior steps' outputs (never the current or
// a later step) and produce the exact template-token grammar.

const input = (name: string, required = false): InputDef => ({ name, required })

const step = (id: string, extra: Partial<BuilderStep> = {}): BuilderStep =>
  ({ id, kind: 'llm', prompt: '', output_format: 'text', ...extra }) as BuilderStep

const def = {
  inputs: [input('topic', true), input('year')],
  steps: [step('s1'), step('s2'), step('s3')],
}

test('lists all inputs + ONLY prior steps for a middle step (index 1)', () => {
  const refs = enumerateRefs(def, 1)
  const tokens = refs.map(r => r.token)
  // Both inputs, then only s1 (the one prior step). NOT s2 (current) or s3 (later).
  assert.deepEqual(tokens, [
    '{{ inputs.topic }}',
    '{{ inputs.year }}',
    '{{ s1.output }}',
  ])
})

test('the first step (index 0) sees inputs but NO prior steps', () => {
  const refs = enumerateRefs(def, 0)
  assert.deepEqual(refs.map(r => r.token), ['{{ inputs.topic }}', '{{ inputs.year }}'])
  assert.ok(refs.every(r => r.group === 'Inputs'))
})

test('the last step sees every earlier step but not itself', () => {
  const refs = enumerateRefs(def, 2)
  const stepTokens = refs.filter(r => r.group === 'Previous steps').map(r => r.token)
  assert.deepEqual(stepTokens, ['{{ s1.output }}', '{{ s2.output }}'])
  assert.ok(!stepTokens.includes('{{ s3.output }}'), 'never references itself')
})

test('input token grammar + required hint', () => {
  const refs = enumerateRefs(def, 0)
  const topic = refs.find(r => r.label === 'topic')
  assert.equal(topic?.token, '{{ inputs.topic }}')
  assert.equal(topic?.group, 'Inputs')
  assert.equal(topic?.hint, 'input · required')
  const year = refs.find(r => r.label === 'year')
  assert.equal(year?.hint, 'input')
})

test('a prior step with a description is labeled "id — description"', () => {
  const d = { inputs: [], steps: [step('s1', { description: 'draft it' }), step('s2')] }
  const refs = enumerateRefs(d, 1)
  const s1 = refs.find(r => r.token === '{{ s1.output }}')
  assert.equal(s1?.label, 's1 — draft it')
})

test('negative currentStepIndex enumerates ALL steps (whole-def mode)', () => {
  const refs = enumerateRefs(def, -1)
  const stepTokens = refs.filter(r => r.group === 'Previous steps').map(r => r.token)
  assert.deepEqual(stepTokens, ['{{ s1.output }}', '{{ s2.output }}', '{{ s3.output }}'])
})

test('empty / undefined def yields no options (no crash)', () => {
  assert.deepEqual(enumerateRefs({}, 0), [])
  assert.deepEqual(enumerateRefs({ inputs: [], steps: [] }, 5), [])
})

test('stepOutputHint reflects kind + output_format', () => {
  assert.equal(stepOutputHint(step('a', { output_format: 'json' })), 'json')
  assert.equal(stepOutputHint(step('a', { output_format: 'text' })), 'text')
  assert.equal(
    stepOutputHint({ id: 'm', kind: 'llm_map', output_format: 'json' } as BuilderStep),
    'json[]',
  )
  assert.equal(stepOutputHint({ id: 's', kind: 'sandbox' } as BuilderStep), 'stdout')
  assert.equal(stepOutputHint({ id: 'e', kind: 'elicit' } as BuilderStep), 'form response')
  assert.equal(stepOutputHint({ id: 't', kind: 'tool' } as BuilderStep), 'tool result')
})
