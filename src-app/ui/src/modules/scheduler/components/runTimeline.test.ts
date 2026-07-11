import { describe, expect, it } from 'vitest'

import type { ScheduledTask, ScheduledTaskRun } from '@/api-client/types'

import {
  changeBadge,
  followupActions,
  runPreviewLine,
  seriesChoices,
} from './runTimeline'

function run(partial: Partial<ScheduledTaskRun>): ScheduledTaskRun {
  return {
    id: 'r1',
    scheduled_task_id: 't1',
    user_id: 'u1',
    trigger: 'schedule',
    status: 'completed',
    fired_at: new Date().toISOString(),
    skipped_tools: [],
    ...partial,
  } as ScheduledTaskRun
}

function task(partial: Partial<ScheduledTask>): ScheduledTask {
  return { id: 't1', name: 'T', target_kind: 'prompt', ...partial } as ScheduledTask
}

describe('changeBadge (TEST-49, ITEM-44)', () => {
  it('maps new_count>0 → NEW ×N / success', () => {
    const b = changeBadge(run({ change_summary_json: { changed: true, new_count: 3 } }))
    expect(b).toEqual({ label: 'NEW ×3', tone: 'success' })
  })
  it('maps changed && new_count==0 → Changed / warning', () => {
    const b = changeBadge(run({ change_summary_json: { changed: true, new_count: 0 } }))
    expect(b).toEqual({ label: 'Changed', tone: 'warning' })
  })
  it('maps !changed → No change / neutral', () => {
    const b = changeBadge(run({ change_summary_json: { changed: false, new_count: 0 } }))
    expect(b).toEqual({ label: 'No change', tone: 'neutral' })
  })
  it('maps a failed run → Failed / error', () => {
    const b = changeBadge(run({ status: 'failed', change_summary_json: null }))
    expect(b).toEqual({ label: 'Failed', tone: 'error' })
  })
  it('returns null for a run with no change summary (pre-migration row)', () => {
    expect(changeBadge(run({ change_summary_json: undefined }))).toBeNull()
  })
  it('clamps the preview to a present string or null', () => {
    expect(runPreviewLine(run({ result_preview: '  hi  ' }))).toBe('hi')
    expect(runPreviewLine(run({ result_preview: '   ' }))).toBeNull()
  })
})

describe('followupActions (TEST-51, ITEM-45)', () => {
  it('prompt + bound → openThread enabled + fork', () => {
    const a = followupActions(task({ target_kind: 'prompt', bound_conversation_id: 'c9' }))
    expect(a.openThread).toBe('enabled')
    expect(a.threadConversationId).toBe('c9')
    expect(a.fork).toBe(true)
  })
  it('prompt + no bound → openThread disabled', () => {
    const a = followupActions(task({ target_kind: 'prompt', bound_conversation_id: undefined }))
    expect(a.openThread).toBe('disabled')
    expect(a.threadConversationId).toBeNull()
  })
  it('workflow → continue only (no thread)', () => {
    const a = followupActions(task({ target_kind: 'workflow' }))
    expect(a.openThread).toBe('none')
    expect(a.fork).toBe(true)
    expect(a.forkLabel).toBe('Continue in chat')
  })
})

describe('seriesChoices (TEST-55, ITEM-47)', () => {
  it('offers {5, 10, all-loaded}, defaulting 5 first', () => {
    const c = seriesChoices(23)
    expect(c.map(o => o.value)).toEqual([5, 10, 23])
    expect(c[0]).toEqual({ label: 'Last 5', value: 5 })
    expect(c[2].label).toBe('All loaded')
  })
  it('keeps all-loaded ≥ 1 even when nothing is loaded', () => {
    expect(seriesChoices(0)[2].value).toBe(1)
  })
})
