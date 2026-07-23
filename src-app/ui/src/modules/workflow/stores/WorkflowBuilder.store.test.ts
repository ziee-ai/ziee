/**
 * TEST-9 — the WorkflowBuilder local store's pure conversion helpers.
 *
 * SCOPE NOTE (sanctioned fallback): the store is a `defineLocalStore`, whose
 * ONLY entry point is `.use()` — a React hook (`useRef` + `useEffect`). This
 * workspace has no `@testing-library/react` / `renderHook` and no precedent for
 * headless-rendering a local store, so its reducer closures (add/reorder/delete
 * step) cannot be instantiated in isolation without standing up a full React
 * runtime. Per the task's fallback, this test pins the PURE helpers the store
 * delegates to instead: `emptyDef`, `toBuilderDef`, `toWorkflowDef` (now
 * exported, behaviour-preserving) — in particular the base-field round-trip
 * (`id` / `description` / `depends_on`) through `StepBase`, and the store's
 * add-step delegation to `createStep`. The reducers themselves are exercised by
 * the module's E2E/integration coverage.
 */
import { describe, expect, it, vi } from 'vitest'

import type { WorkflowDef } from '@/api-client/types'

vi.mock('@/api-client', () => ({ ApiClient: {} }))
vi.mock('@/core/permissions', () => ({ hasPermissionNow: () => true }))

import { createStep } from '../components/builder/stepForms'
import {
  type BuilderDef,
  emptyDef,
  toBuilderDef,
  toWorkflowDef,
} from './WorkflowBuilder.store'

describe('emptyDef', () => {
  it('is a blank, mutable definition', () => {
    const d = emptyDef()
    expect(d.inputs).toEqual([])
    expect(d.steps).toEqual([])
    // Distinct instances (no shared array reference between sessions).
    expect(emptyDef().steps).not.toBe(d.steps)
  })
})

describe('toBuilderDef / toWorkflowDef round-trip', () => {
  it('round-trips base fields (id / description / depends_on) through StepBase', () => {
    const wire: WorkflowDef = {
      $schema: 'https://ziee/workflow.schema.json',
      max_runtime_secs: 600,
      inputs: [{ name: 'topic', required: true }],
      steps: [
        {
          // base fields serde-flatten keeps on the wire but the generated
          // StepDef type drops — the whole point of the BuilderStep narrowing.
          id: 's1',
          description: 'first step',
          depends_on: [],
          kind: 'llm',
          prompt: 'hello',
          output_format: 'text',
        },
        {
          id: 's2',
          description: 'second step',
          depends_on: ['s1'],
          kind: 'sandbox',
          run: 'echo hi',
          timeout_ms: 30000,
        },
      ] as never,
    }

    const builder = toBuilderDef(wire)
    // Base fields survive the wire → builder narrowing.
    expect(builder.steps.map(s => s.id)).toEqual(['s1', 's2'])
    expect(builder.steps[1].description).toBe('second step')
    expect(builder.steps[1].depends_on).toEqual(['s1'])

    const back = toWorkflowDef(builder)
    // builder → wire preserves base fields + config + top-level metadata.
    expect(back.steps).toEqual(wire.steps)
    expect(back.inputs).toEqual(wire.inputs)
    expect(back.$schema).toBe(wire.$schema)
    expect(back.max_runtime_secs).toBe(600)
  })

  it('toWorkflowDef omits absent optional metadata (no $schema / max_runtime_secs keys)', () => {
    const def: BuilderDef = { inputs: [], steps: [] }
    const wire = toWorkflowDef(def)
    expect('$schema' in wire).toBe(false)
    expect('max_runtime_secs' in wire).toBe(false)
    expect(wire.inputs).toEqual([])
    expect(wire.steps).toEqual([])
  })

  it('toBuilderDef tolerates a wire def with no inputs/steps', () => {
    const builder = toBuilderDef({} as WorkflowDef)
    expect(builder.inputs).toEqual([])
    expect(builder.steps).toEqual([])
  })
})

describe('add-step delegation (the reducer builds via createStep)', () => {
  it('createStep produces collision-free ids against the working def', () => {
    // Mirrors `addStep`, which calls createStep(kind, def.steps.map(s => s.id)).
    const def: BuilderDef = { inputs: [], steps: [] }
    const a = createStep('agent', def.steps.map(s => s.id))
    def.steps.push(a)
    const b = createStep('agent', def.steps.map(s => s.id))
    def.steps.push(b)
    expect(def.steps.map(s => s.id)).toEqual(['agent_1', 'agent_2'])
    // Each added step round-trips cleanly back to the wire form.
    expect(toWorkflowDef(def).steps).toEqual(def.steps)
  })
})
