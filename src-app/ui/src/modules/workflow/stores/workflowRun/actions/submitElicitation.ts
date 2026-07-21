import { ApiClient } from '@/api-client'
import type { WorkflowRunGet, WorkflowRunSet } from '../state'

export default (set: WorkflowRunSet, _get: WorkflowRunGet) =>
  async (runId: string, elicitationId: string, response: any) => {
    set(draft => {
      draft.submittingElicit[runId] = true
    })
    try {
      await ApiClient.Workflow.submitElicit({
        run_id: runId,
        elicitation_id: elicitationId,
        response,
      })
      set(draft => {
        const v = draft.runs[runId]
        if (v) v.pendingElicitation = undefined
      })
    } catch (e) {
      // M-7: surface a failed submission instead of rejecting unhandled.
      set(draft => {
        const v = draft.runs[runId]
        if (v) v.error = `Failed to submit response: ${String(e)}`
      })
    } finally {
      set(draft => {
        delete draft.submittingElicit[runId]
      })
    }
  }
