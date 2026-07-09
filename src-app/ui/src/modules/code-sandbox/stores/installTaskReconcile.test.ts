import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { InstallTaskState } from '@/api-client/types'
import { reconcileInitialTask } from './installTaskReconcile.ts'

// `over` is loosely typed: the wire sends `phase`/`message` as `null` (the
// generated type models them as `string | undefined`), and this test mirrors the
// real payload shape.
function task(over: Record<string, unknown>): InstallTaskState {
  return {
    task_id: 't1',
    version: '1.0.0-alpha',
    arch: 'x86_64',
    flavor: 'full',
    package: 'squashfs',
    status: 'running',
    phase: null,
    message: null,
    started_at: '2026-07-08T00:00:00Z',
    completed_at: null,
    artifact_id: null,
    bytes_downloaded: null,
    duration_ms: null,
    error: null,
    ...over,
  } as unknown as InstallTaskState
}

// ── The install-progress race: the POST reply must not clobber SSE progress ──

test('seeds the initial 202 task when the SSE has not created one yet', () => {
  const initial = task({})
  assert.equal(reconcileInitialTask(undefined, initial), initial)
})

test('does NOT downgrade an SSE-advanced task with the late initial (phase null)', () => {
  // The SSE already reported the long download in progress…
  const advanced = task({ phase: 'downloading', message: 'downloading https://…/full.squashfs' })
  // …then the POST finally resolves with its initial phase-less state.
  const result = reconcileInitialTask(advanced, task({ phase: null, message: null }))
  assert.equal(result, advanced)
  assert.equal(result.phase, 'downloading') // not clobbered back to "queued"
  assert.equal(result.message, 'downloading https://…/full.squashfs')
})

test('does not resurrect a terminal SSE task with a running initial', () => {
  const done = task({ status: 'completed', phase: 'complete' })
  const result = reconcileInitialTask(done, task({ status: 'running', phase: null }))
  assert.equal(result.status, 'completed')
  assert.equal(result.phase, 'complete')
})
