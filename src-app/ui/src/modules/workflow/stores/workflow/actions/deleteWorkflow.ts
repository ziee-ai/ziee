import { ApiClient } from '@/api-client'
import type { WorkflowGet, WorkflowSet } from '../state'

export default (set: WorkflowSet, _get: WorkflowGet) => {
  return async (id: string): Promise<void> => {
    set(draft => {
      draft.operationsLoading[id] = true
      draft.error = null
    })
    try {
      await ApiClient.Workflow.delete({ id })
      set(draft => {
        draft.workflows = draft.workflows.filter(w => w.id !== id)
        delete draft.operationsLoading[id]
      })
    } catch (error) {
      set(draft => {
        delete draft.operationsLoading[id]
        draft.error = error instanceof Error ? error.message : 'Failed to delete workflow'
      })
      throw error
    }
  }
}
