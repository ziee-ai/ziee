import { test } from 'node:test'
import assert from 'node:assert/strict'
import { TOOL_STATUS, toolStatusKey, toolStatusOf } from './tool-status.ts'

// The defining invariant (finding #2): a user-cancelled tool call must NEVER be
// presented like a failed one. Color is the single source of truth for that
// distinction, so cancelled and failed must differ in color.
test('cancelled color differs from failed color', () => {
  assert.notEqual(TOOL_STATUS.cancelled.color, TOOL_STATUS.failed.color)
})

test('cancelled is neutral muted-gray, not destructive red', () => {
  assert.equal(TOOL_STATUS.cancelled.color, 'text-muted-foreground')
  assert.notEqual(TOOL_STATUS.cancelled.color, 'text-destructive')
  assert.notEqual(TOOL_STATUS.cancelled.tone, TOOL_STATUS.failed.tone)
})

test('cancelled and failed use distinct icons (failed owns the red X)', () => {
  assert.notEqual(TOOL_STATUS.cancelled.icon, TOOL_STATUS.failed.icon)
  // No other status may reuse the failed XCircle icon — a red X unambiguously
  // means "errored".
  for (const key of Object.keys(TOOL_STATUS) as (keyof typeof TOOL_STATUS)[]) {
    if (key === 'failed') continue
    assert.notEqual(
      TOOL_STATUS[key].icon,
      TOOL_STATUS.failed.icon,
      `status "${key}" must not reuse the failed icon`,
    )
  }
})

test('destructive red is exclusive to failed', () => {
  for (const key of Object.keys(TOOL_STATUS) as (keyof typeof TOOL_STATUS)[]) {
    if (key === 'failed') continue
    assert.notEqual(
      TOOL_STATUS[key].color,
      'text-destructive',
      `status "${key}" must not use destructive red`,
    )
  }
})

test('toolStatusKey normalizes every surface vocabulary', () => {
  assert.equal(toolStatusKey('completed'), 'success')
  assert.equal(toolStatusKey('success'), 'success')
  assert.equal(toolStatusKey('error'), 'failed')
  assert.equal(toolStatusKey('failed'), 'failed')
  assert.equal(toolStatusKey('cancelled'), 'cancelled')
  assert.equal(toolStatusKey('timeout'), 'timeout')
  assert.equal(toolStatusKey('pending_approval'), 'pending-approval')
  assert.equal(toolStatusKey('started'), 'running')
  assert.equal(toolStatusKey('pending'), 'running')
  assert.equal(toolStatusKey('anything-unknown'), 'running')
  // An explicit error flag forces failed regardless of the raw status.
  assert.equal(toolStatusKey('completed', true), 'failed')
})

test('toolStatusOf resolves a descriptor and never reds a cancel', () => {
  assert.equal(toolStatusOf('cancelled'), TOOL_STATUS.cancelled)
  assert.notEqual(toolStatusOf('cancelled').color, toolStatusOf('failed').color)
})
