import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mapDiscoveredModelToForm } from './discoveredModelForm.ts'

// TEST-1 — the picker's auto-fill mapping (DiscoveredModel -> form fields).

test('maps supports_* capability flags and context onto the form', () => {
  const f = mapDiscoveredModelToForm({
    id: 'anthropic/claude-sonnet-5',
    display_name: 'Claude Sonnet 5',
    supports_vision: true,
    supports_tool_use: true,
    supports_embeddings: false,
    supports_chat: true,
    context_length: 200000,
  })
  assert.equal(f.display_name, 'Claude Sonnet 5')
  assert.equal(f.vision, true)
  assert.equal(f.tools, true)
  assert.equal(f.text_embedding, false)
  assert.equal(f.chat, true)
  assert.equal(f.context_length, 200000)
})

test('falls back to the id when the model has no display_name', () => {
  const f = mapDiscoveredModelToForm({ id: 'some/model', supports_chat: true })
  assert.equal(f.display_name, 'some/model')
})

test('coerces undefined capability flags to false (never undefined)', () => {
  const f = mapDiscoveredModelToForm({ id: 'x', supports_chat: false })
  assert.equal(f.vision, false)
  assert.equal(f.tools, false)
  assert.equal(f.text_embedding, false)
  assert.equal(f.chat, false)
  assert.equal(f.context_length, undefined)
})

test('an embeddings-capable model preserves the embedding flag', () => {
  const f = mapDiscoveredModelToForm({
    id: 'text-embedding-3-small',
    supports_embeddings: true,
    supports_chat: false,
  })
  assert.equal(f.text_embedding, true)
  assert.equal(f.chat, false)
})
