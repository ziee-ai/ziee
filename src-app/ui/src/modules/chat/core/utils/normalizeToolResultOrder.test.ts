import { test } from 'node:test'
import assert from 'node:assert/strict'
import { normalizeToolResultOrder } from './normalizeToolResultOrder.ts'
import type { MessageContent } from '@/api-client/types'

// Minimal block builders — the function reads only `content_type`,
// `content.id` (tool_use), and `content.tool_use_id` (tool_result).
const use = (id: string): MessageContent =>
  ({
    id: `blk-${id}`,
    content_type: 'tool_use',
    content: { type: 'tool_use', id },
  }) as unknown as MessageContent

const result = (useId: string, links: unknown[] = []): MessageContent =>
  ({
    id: `res-${useId}`,
    content_type: 'tool_result',
    content: { type: 'tool_result', tool_use_id: useId, resource_links: links },
  }) as unknown as MessageContent

const text = (t: string): MessageContent =>
  ({
    id: `txt-${t}`,
    content_type: 'text',
    content: { type: 'text', text: t },
  }) as unknown as MessageContent

const ids = (blocks: MessageContent[]) => blocks.map(b => b.id)

test('streaming order: an artifact result appended after a text block is pulled adjacent to its tool_use', () => {
  // [use_A, use_B, text, result_B(artifact)] — the exact escape scenario.
  const input = [use('A'), use('B'), text('done'), result('B', [{ file_id: 'f1' }])]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['blk-A', 'blk-B', 'res-B', 'txt-done'])
})

test('reload/persisted order with an already-adjacent result is unchanged', () => {
  const input = [use('A'), result('A'), text('mid'), use('B'), result('B')]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['blk-A', 'res-A', 'txt-mid', 'blk-B', 'res-B'])
})

test('parallel tools: each result attaches to its own tool_use', () => {
  // Both uses first, then both results (a common parallel-call shape).
  const input = [use('A'), use('B'), result('A'), result('B')]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['blk-A', 'res-A', 'blk-B', 'res-B'])
})

test('lone tool: result after trailing text is placed adjacent to the tool_use', () => {
  const input = [use('A'), text('here'), result('A', [{ file_id: 'f1' }])]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['blk-A', 'res-A', 'txt-here'])
})

test('orphan result (no matching tool_use present) keeps its position', () => {
  const input = [text('a'), result('MISSING'), text('b')]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['txt-a', 'res-MISSING', 'txt-b'])
})

test('non-tool blocks keep their relative order', () => {
  const input = [text('1'), use('A'), text('2'), result('A'), text('3')]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['txt-1', 'blk-A', 'res-A', 'txt-2', 'txt-3'])
})

test('multiple results for one tool_use keep their relative order, adjacent to the use', () => {
  const r1 = result('A')
  r1.id = 'res-A-1'
  const r2 = result('A')
  r2.id = 'res-A-2'
  const input = [use('A'), text('x'), r1, r2]
  const out = normalizeToolResultOrder(input)
  assert.deepEqual(ids(out), ['blk-A', 'res-A-1', 'res-A-2', 'txt-x'])
})

test('pure: input array and elements are not mutated; output is a permutation', () => {
  const input = [use('A'), use('B'), text('done'), result('B')]
  const snapshotIds = ids(input)
  const out = normalizeToolResultOrder(input)
  // input untouched
  assert.deepEqual(ids(input), snapshotIds)
  assert.equal(input.length, snapshotIds.length)
  // output is a permutation (same multiset of element references)
  assert.equal(out.length, input.length)
  assert.deepEqual([...ids(out)].sort(), [...snapshotIds].sort())
})

test('idempotent: normalizing a normalized array yields the same order', () => {
  const input = [use('A'), use('B'), text('done'), result('B')]
  const once = normalizeToolResultOrder(input)
  const twice = normalizeToolResultOrder(once)
  assert.deepEqual(ids(twice), ids(once))
})

test('no tool_results: returns the same array reference (fast identity path)', () => {
  const input = [text('a'), use('A'), text('b')]
  const out = normalizeToolResultOrder(input)
  assert.equal(out, input)
})
