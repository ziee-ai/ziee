import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  STEP_KINDS,
  type StepKind,
  buildStepZodSchema,
  configErrors,
  createStep,
} from './stepForms.ts'

// TEST-13 — the per-kind zod-schema builder / `createStep` / `configErrors`.
// Pure logic: for every StepConfig kind, `createStep` yields a valid default,
// the schema rejects out-of-range / missing-required fields, and the removed
// tools picker regression is guarded (llm / llm_map set no non-empty `tools`).

test('createStep yields a schema-VALID default for every kind', () => {
  for (const kind of STEP_KINDS) {
    const step = createStep(kind, [])
    assert.equal(step.kind, kind, `kind tag preserved for ${kind}`)
    // A brand-new step should still fail required-text validation (empty prompt/
    // run/message/server) but NOT fail on any numeric/range/enum default — the
    // number + enum defaults are always in range. So: the ONLY error keys, if
    // any, are the required-text fields, never a numeric/enum default.
    const errs = configErrors(step)
    const numericOrEnumKeys = [
      'max_steps',
      'max_parallel',
      'max_retries',
      'timeout_ms',
      'output_format',
    ]
    for (const k of numericOrEnumKeys) {
      assert.ok(
        !(k in errs),
        `${kind}: default must satisfy ${k} (got error: ${errs[k]})`,
      )
    }
  }
})

test('createStep gives a fully-valid step once required text is filled', () => {
  // Fill each kind's required text fields and assert zero config errors — proves
  // the defaults for the non-text fields are all in range.
  const filled: Record<StepKind, Record<string, unknown>> = {
    agent: { prompt: 'do the thing' },
    llm: { prompt: 'answer this' },
    llm_map: { prompt: 'p', for_each: '{{ inputs.list }}', item_var: 'item' },
    sandbox: { run: 'echo hi' },
    elicit: { message: 'your name?' },
    tool: { server: 'web', tool: 'web_search' },
  }
  for (const kind of STEP_KINDS) {
    const step = { ...createStep(kind, []), ...filled[kind] } as never
    assert.deepEqual(
      configErrors(step),
      {},
      `${kind}: a required-fields-filled default must be fully valid`,
    )
  }
})

test('unique step ids: createStep avoids collisions with existing ids', () => {
  const a = createStep('llm', [])
  const b = createStep('llm', [a.id])
  assert.equal(a.id, 'llm_1')
  assert.equal(b.id, 'llm_2')
  assert.notEqual(a.id, b.id)
})

test('schema rejects a MISSING required field (empty prompt)', () => {
  const step = createStep('llm', []) // prompt: ''
  const errs = configErrors(step)
  assert.ok('prompt' in errs, 'empty prompt must be flagged')
})

test('schema rejects an OUT-OF-RANGE numeric field', () => {
  // agent max_steps must be >= 1.
  const agent = { ...createStep('agent', []), prompt: 'x', max_steps: 0 } as never
  assert.ok('max_steps' in configErrors(agent), 'max_steps 0 must fail min(1)')

  // llm_map max_parallel is capped at the hard cap (20).
  const map = {
    ...createStep('llm_map', []),
    prompt: 'p',
    for_each: 'l',
    item_var: 'i',
    max_parallel: 999,
  } as never
  assert.ok('max_parallel' in configErrors(map), 'max_parallel 999 must fail max cap')

  // llm_map max_retries cannot be negative.
  const retries = {
    ...createStep('llm_map', []),
    prompt: 'p',
    for_each: 'l',
    item_var: 'i',
    max_retries: -1,
  } as never
  assert.ok('max_retries' in configErrors(retries), 'negative max_retries must fail')

  // sandbox timeout_ms must be >= 1.
  const sandbox = { ...createStep('sandbox', []), run: 'echo', timeout_ms: 0 } as never
  assert.ok('timeout_ms' in configErrors(sandbox), 'sandbox timeout 0 must fail')
})

test('schema rejects an invalid enum (output_format)', () => {
  const step = { ...createStep('agent', []), prompt: 'x', output_format: 'xml' } as never
  assert.ok('output_format' in configErrors(step), 'unknown output_format must fail')
})

test('REGRESSION: llm / llm_map createStep sets NO non-empty `tools`', () => {
  // The tools picker was removed; the backend rejects a non-empty `tools` on an
  // llm/llm_map step. Guard that the default omits it (or leaves it empty).
  for (const kind of ['llm', 'llm_map'] as const) {
    const step = createStep(kind, []) as unknown as Record<string, unknown>
    const tools = step.tools
    const nonEmpty = Array.isArray(tools) && tools.length > 0
    assert.equal(
      nonEmpty,
      false,
      `${kind} default must not carry a non-empty tools field (got ${JSON.stringify(tools)})`,
    )
  }
})

test('buildStepZodSchema returns a schema for each kind (no throw)', () => {
  for (const kind of STEP_KINDS) {
    const schema = buildStepZodSchema(kind)
    assert.ok(schema, `schema built for ${kind}`)
    // safeParse never throws even on garbage input.
    assert.doesNotThrow(() => schema.safeParse({ nonsense: true } as never))
  }
})
