import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  runToolUseIds,
  hasArtifactInRun,
  shouldAutoOpen,
  deriveGroupOpen,
  resolveArtifactToolUseId,
} from './toolRun.ts'
import type { MessageContent } from '@/api-client/types'
import type { McpToolCall } from '@/modules/mcp/stores/McpComposer.store'

const use = (id: string): MessageContent =>
  ({
    id: `blk-${id}`,
    content_type: 'tool_use',
    content: { type: 'tool_use', id },
  }) as unknown as MessageContent

const result = (useId: string, links?: unknown[]): MessageContent =>
  ({
    id: `res-${useId}`,
    content_type: 'tool_result',
    content: {
      type: 'tool_result',
      tool_use_id: useId,
      ...(links !== undefined ? { resource_links: links } : {}),
    },
  }) as unknown as MessageContent

const call = (
  id: string,
  status: McpToolCall['status'],
): McpToolCall => ({
  tool_use_id: id,
  server: 's',
  tool_name: 't',
  status,
})

// ── runToolUseIds ──────────────────────────────────────────────────────────
test('runToolUseIds returns only the tool_use ids in order', () => {
  const run = [use('A'), result('A'), use('B'), result('B')]
  assert.deepEqual(runToolUseIds(run), ['A', 'B'])
})

// ── hasArtifactInRun ───────────────────────────────────────────────────────
test('hasArtifactInRun is true when a tool_result carries ≥1 resource_link', () => {
  assert.equal(hasArtifactInRun([use('A'), result('A', [{ file_id: 'f1' }])]), true)
})

test('hasArtifactInRun is false for empty or absent resource_links', () => {
  assert.equal(hasArtifactInRun([use('A'), result('A', [])]), false)
  assert.equal(hasArtifactInRun([use('A'), result('A')]), false)
  assert.equal(hasArtifactInRun([use('A')]), false)
})

// ── shouldAutoOpen (the latch trigger) ─────────────────────────────────────
test('shouldAutoOpen is true when running or artifact, false otherwise', () => {
  assert.equal(shouldAutoOpen({ hasRunning: true, hasArtifact: false }), true)
  assert.equal(shouldAutoOpen({ hasRunning: false, hasArtifact: true }), true)
  assert.equal(shouldAutoOpen({ hasRunning: false, hasArtifact: false }), false)
})

// ── deriveGroupOpen (the render decision) ──────────────────────────────────
test('deriveGroupOpen: pending approval forces open even when userOpen is false', () => {
  assert.equal(deriveGroupOpen({ hasPendingApproval: true, userOpen: false }), true)
})

test('deriveGroupOpen: without pending approval it follows userOpen (collapsible)', () => {
  assert.equal(deriveGroupOpen({ hasPendingApproval: false, userOpen: true }), true)
  assert.equal(deriveGroupOpen({ hasPendingApproval: false, userOpen: false }), false)
})

// ── resolveArtifactToolUseId (misattribution guard) ────────────────────────
test('resolveArtifactToolUseId prefers the explicit event tool_use_id', () => {
  const contents = [use('A'), use('B')]
  const store = new Map<string, McpToolCall>()
  assert.equal(resolveArtifactToolUseId(contents, store, 'B'), 'B')
})

test('resolveArtifactToolUseId falls back to the sole tool_use when no event id', () => {
  const contents = [use('A')]
  const store = new Map<string, McpToolCall>()
  assert.equal(resolveArtifactToolUseId(contents, store, undefined), 'A')
})

test('resolveArtifactToolUseId disambiguates via a single in-flight store call', () => {
  const contents = [use('A'), use('B')]
  const store = new Map<string, McpToolCall>([
    ['A', call('A', 'completed')],
    ['B', call('B', 'started')],
  ])
  assert.equal(resolveArtifactToolUseId(contents, store, null), 'B')
})

test('resolveArtifactToolUseId returns null when parallel tools are ambiguous (never guesses last)', () => {
  const contents = [use('A'), use('B')]
  const store = new Map<string, McpToolCall>([
    ['A', call('A', 'started')],
    ['B', call('B', 'started')],
  ])
  assert.equal(resolveArtifactToolUseId(contents, store, undefined), null)
})

test('resolveArtifactToolUseId ignores an in-flight call NOT in this message (no cross-conversation capture)', () => {
  const contents = [use('A'), use('B')] // ambiguous within the message
  // The only in-flight store call belongs to a tool_use NOT in this message.
  const store = new Map<string, McpToolCall>([
    ['OTHER', call('OTHER', 'started')],
  ])
  assert.equal(resolveArtifactToolUseId(contents, store, undefined), null)
})
