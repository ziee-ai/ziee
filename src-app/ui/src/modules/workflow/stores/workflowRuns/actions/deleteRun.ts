import { ApiClient } from '@/api-client'
import type { WorkflowRunsGet, WorkflowRunsSet } from '../state'

export default (set: WorkflowRunsSet, _get: WorkflowRunsGet) =>
  async (runId: string, workflowId: string) => {
    try {
      set(d => {
        d.deleting[runId] = true
      })
      await ApiClient.Workflow.deleteRun({ run_id: runId })
      set(d => {
        d.deleting[runId] = false
        if (d.runs[workflowId]) {
          d.runs[workflowId] = d.runs[workflowId].filter(r => r.id !== runId)
        }
      })
    } catch (e) {
      set(d => {
        d.deleting[runId] = false
      })
      throw e
    }
  }
