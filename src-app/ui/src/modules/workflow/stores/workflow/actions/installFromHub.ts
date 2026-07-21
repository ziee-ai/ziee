import { ApiClient } from '@/api-client'
import type { Workflow, WorkflowSet } from '../state'

export default (set: WorkflowSet) => {
  return async (hubId: string): Promise<Workflow> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const response = await ApiClient.Hub.createWorkflowFromHub({ hub_id: hubId })
      set(draft => {
        draft.workflows.push(response.workflow)
        draft.creating = false
      })
      return response.workflow
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error = error instanceof Error ? error.message : 'Failed to install workflow'
      })
      throw error
    }
  }
}
