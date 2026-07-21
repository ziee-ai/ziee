import { ApiClient } from '@/api-client'
import type { WorkflowRunGet, WorkflowRunSet } from '../state'

export default (set: WorkflowRunSet, _get: WorkflowRunGet) =>
  async (runId: string) => {
    set(draft => {
      draft.cancelling[runId] = true
    })
    try {
      await ApiClient.Workflow.cancelRun({ run_id: runId })
    } catch (e) {
      // M-7: callers fire-and-forget via `void`, so surface here rather than
      // reject unhandled. Show the failure in the run's error banner.
      set(draft => {
        const v = draft.runs[runId]
        if (v) v.error = `Failed to cancel run: ${String(e)}`
      })
    } finally {
      set(draft => {
        delete draft.cancelling[runId]
      })
    }
  }
