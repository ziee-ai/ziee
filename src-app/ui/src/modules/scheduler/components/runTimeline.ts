import type { ScheduledTask, ScheduledTaskRun } from '@/api-client/types'

/** DEC-20 (fixed UX constant): run-history page size for the runs panel. */
export const RUNS_PAGE_SIZE = 10
/** DEC-22 (fixed UX constant): default "Discuss recent runs" count. */
export const SERIES_DEFAULT_N = 5

export type ChangeTone = 'success' | 'warning' | 'neutral' | 'error'
export interface ChangeBadge {
  label: string
  tone: ChangeTone
}

/**
 * ITEM-44: map a run's status + change summary to the "what changed" badge.
 * Returns `null` for a run with no change summary (e.g. a pre-migration-155 row)
 * so the row renders neutrally instead of a misleading badge. (TEST-49)
 */
export function changeBadge(run: ScheduledTaskRun): ChangeBadge | null {
  if (run.status === 'failed') return { label: 'Failed', tone: 'error' }
  const cs = run.change_summary_json as
    | { changed?: boolean; new_count?: number }
    | null
    | undefined
  if (!cs) return null
  const newCount = typeof cs.new_count === 'number' ? cs.new_count : 0
  if (newCount > 0) return { label: `NEW ×${newCount}`, tone: 'success' }
  if (cs.changed) return { label: 'Changed', tone: 'warning' }
  return { label: 'No change', tone: 'neutral' }
}

/** One-line preview text for the timeline row, or null when absent. */
export function runPreviewLine(run: ScheduledTaskRun): string | null {
  const p = run.result_preview?.trim()
  return p && p.length > 0 ? p : null
}

export type OpenThreadState = 'enabled' | 'disabled' | 'none'
export interface FollowupActions {
  /** enabled: a prompt task with a bound conversation; disabled: prompt, not yet
   *  fired; none: a workflow task (no thread). */
  openThread: OpenThreadState
  threadConversationId: string | null
  /** the fork affordance is always present (opens a fresh seeded conversation). */
  fork: boolean
  forkLabel: string
}

/**
 * ITEM-45 (DEC-21): resolve a task's follow-up actions. A prompt task's home is
 * its bound conversation ("Open thread"), with the fork kept as a secondary "New
 * side chat"; a workflow task has no thread, so its only follow-up is the fork,
 * labelled "Continue in chat". (TEST-51)
 */
export function followupActions(task: ScheduledTask): FollowupActions {
  if (task.target_kind === 'prompt') {
    const bound = task.bound_conversation_id ?? null
    return {
      openThread: bound ? 'enabled' : 'disabled',
      threadConversationId: bound,
      fork: true,
      forkLabel: 'New side chat',
    }
  }
  return {
    openThread: 'none',
    threadConversationId: null,
    fork: true,
    forkLabel: 'Continue in chat',
  }
}

export interface SeriesChoice {
  label: string
  value: number
}

/**
 * ITEM-47 (DEC-22): the "Discuss recent runs" chooser options — {5, 10,
 * all-loaded}. "All loaded" folds in every run the panel has fetched. (TEST-55)
 */
export function seriesChoices(loadedCount: number): SeriesChoice[] {
  const raw: SeriesChoice[] = [
    { label: 'Last 5', value: 5 },
    { label: 'Last 10', value: 10 },
    { label: 'All loaded', value: Math.max(loadedCount, 1) },
  ]
  // Dedupe by value so "All loaded" doesn't collide with "Last 5/10" (duplicate
  // Select keys) when exactly 5 or 10 runs are loaded.
  const seen = new Set<number>()
  return raw.filter(o => {
    if (seen.has(o.value)) return false
    seen.add(o.value)
    return true
  })
}
