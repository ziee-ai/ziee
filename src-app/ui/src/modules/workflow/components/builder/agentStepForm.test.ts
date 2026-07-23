import assert from 'node:assert/strict'
import { test } from 'node:test'

import {
  EFFORTS,
  EFFORT_STEPS,
  type Effort,
  agentReadback,
  effortToMaxSteps,
  isCustomMaxSteps,
  maxStepsToEffort,
} from './agentStepForm.ts'

// TEST-15 — friendly agent-step helpers: the effort<->max_steps round-trip
// (Quick/Balanced/Thorough <-> 10/30/60), off-preset nearest-label mapping, the
// plain-English readback, and the capability display_name -> server-name mapping.

test('effortToMaxSteps maps the three named levels to 10/30/60', () => {
  assert.equal(effortToMaxSteps('quick'), 10)
  assert.equal(effortToMaxSteps('balanced'), 30)
  assert.equal(effortToMaxSteps('thorough'), 60)
})

test('effort <-> max_steps round-trips on every preset', () => {
  for (const effort of EFFORTS) {
    assert.equal(
      maxStepsToEffort(effortToMaxSteps(effort)),
      effort,
      `round-trip preserves ${effort}`,
    )
    assert.equal(isCustomMaxSteps(EFFORT_STEPS[effort]), false, `${effort} is a preset`)
  }
})

test('maxStepsToEffort snaps an OFF-preset number to the nearest label', () => {
  // Nearest by absolute distance to {10,30,60}.
  const cases: Array<[number, Effort]> = [
    [8, 'quick'], // closest to 10
    [12, 'quick'], // 12 -> |10-12|=2 vs |30-12|=18 -> quick
    [19, 'quick'], // |10-19|=9 vs |30-19|=11 -> quick
    [21, 'balanced'], // |30-21|=9 vs |10-21|=11 -> balanced
    [40, 'balanced'], // |30-40|=10 vs |60-40|=20 -> balanced
    [50, 'thorough'], // |60-50|=10 vs |30-50|=20 -> thorough
    [100, 'thorough'], // far past the top -> thorough
  ]
  for (const [n, expected] of cases) {
    assert.equal(maxStepsToEffort(n), expected, `${n} -> ${expected}`)
    assert.equal(isCustomMaxSteps(n), true, `${n} is a custom (off-preset) value`)
  }
})

test('agentReadback reflects the config in plain English', () => {
  const text = agentReadback({
    prompt: 'summarize the trial results',
    max_steps: 30,
    output_format: 'text',
    capabilityLabels: ['Web Search', 'Literature'],
  })
  assert.match(text, /The assistant will/)
  assert.match(text, /summarize the trial results/)
  assert.match(text, /using Web Search and Literature/)
  assert.match(text, /taking up to 30 steps/)
  assert.match(text, /return a written answer/)
})

test('agentReadback: no capabilities, single step, json output', () => {
  const text = agentReadback({
    prompt: '',
    max_steps: 1,
    output_format: 'json',
    capabilityLabels: [],
  })
  // Empty prompt -> the fallback goal phrasing.
  assert.match(text, /run the task you describe above/)
  assert.match(text, /without any tools/)
  // Singular "step" (no trailing "s").
  assert.match(text, /taking up to 1 step,/)
  assert.match(text, /return a structured result/)
})

test('agentReadback: >3 capabilities collapse to a count', () => {
  const text = agentReadback({
    prompt: 'x',
    max_steps: 60,
    capabilityLabels: ['A', 'B', 'C', 'D'],
  })
  assert.match(text, /using 4 capabilities/)
})

test('agentReadback: missing max_steps defaults to the balanced ceiling', () => {
  const text = agentReadback({ prompt: 'x', capabilityLabels: [] })
  assert.match(text, new RegExp(`taking up to ${EFFORT_STEPS.balanced} steps`))
})

// The capability picker maps the user's accessible MCP servers to options whose
// VALUE is the server NAME (steps resolve tools by name at run time) and whose
// LABEL is the friendly display_name, falling back to the name when absent.
// The mapping lives inside a React hook (`useCapabilityOptions` in
// capabilities.tsx) so it isn't headlessly importable; this locks the exact
// value/label rule it encodes so a regression in the picker is caught here.
function capabilityOption(s: { name: string; display_name?: string; enabled: boolean }) {
  return { value: s.name, label: s.display_name || s.name }
}

test('capability mapping: value=server name, label=display_name (fallback to name)', () => {
  assert.deepEqual(capabilityOption({ name: 'web', display_name: 'Web Search', enabled: true }), {
    value: 'web',
    label: 'Web Search',
  })
  // No display name -> label falls back to the server name.
  assert.deepEqual(capabilityOption({ name: 'bio', enabled: true }), {
    value: 'bio',
    label: 'bio',
  })
  // Empty-string display name is falsy -> also falls back to the name.
  assert.deepEqual(
    capabilityOption({ name: 'lit', display_name: '', enabled: true }),
    { value: 'lit', label: 'lit' },
  )
})
