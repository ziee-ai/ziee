import assert from 'node:assert/strict'
import { test } from 'node:test'

import type { ProviderWithModels } from '@/api-client/types'
import { buildModelOptions } from './taskTargetOptions.ts'

// TEST-5 (ITEM-1) — the model-picker's grouped-options builder. Pure mapping of
// the user's accessible providers into `{ label, options:[{label,value}] }`,
// using model DISPLAY names for labels and the model id for value.

const provider = (
  name: string,
  models: Array<{ id: string; name: string; display_name?: string; enabled?: boolean }>,
): ProviderWithModels =>
  ({
    id: `prov-${name}`,
    name,
    llm_models: models.map(m => ({
      id: m.id,
      name: m.name,
      display_name: m.display_name,
      enabled: m.enabled ?? true,
    })),
  }) as unknown as ProviderWithModels

test('maps providers into grouped options using display name for label + id for value', () => {
  const groups = buildModelOptions([
    provider('OpenAI', [
      { id: 'm-gpt', name: 'gpt-4o', display_name: 'GPT-4o' },
    ]),
    provider('Anthropic', [
      { id: 'm-sonnet', name: 'claude-sonnet', display_name: 'Claude Sonnet' },
    ]),
  ])

  assert.deepEqual(groups, [
    { label: 'OpenAI', options: [{ label: 'GPT-4o', value: 'm-gpt' }] },
    {
      label: 'Anthropic',
      options: [{ label: 'Claude Sonnet', value: 'm-sonnet' }],
    },
  ])
})

test('falls back to model name when display_name is absent', () => {
  const [group] = buildModelOptions([
    provider('Local', [{ id: 'm1', name: 'llama-3', display_name: undefined }]),
  ])
  assert.equal(group.options[0].label, 'llama-3')
  assert.equal(group.options[0].value, 'm1')
})

test('drops disabled models and then any provider left with no options', () => {
  const groups = buildModelOptions([
    provider('AllDisabled', [
      { id: 'x', name: 'x', enabled: false },
      { id: 'y', name: 'y', enabled: false },
    ]),
    provider('Mixed', [
      { id: 'on', name: 'on', enabled: true },
      { id: 'off', name: 'off', enabled: false },
    ]),
  ])

  // The empty-model provider group is dropped entirely.
  assert.equal(groups.length, 1)
  assert.equal(groups[0].label, 'Mixed')
  assert.deepEqual(
    groups[0].options.map(o => o.value),
    ['on'],
  )
})

test('undefined / empty providers yield an empty list (no crash)', () => {
  assert.deepEqual(buildModelOptions(undefined), [])
  assert.deepEqual(buildModelOptions([]), [])
})
