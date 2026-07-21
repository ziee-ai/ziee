import { ApiClient } from '@/api-client'
import type { Workflow } from '@/api-client/types'
import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, _get: SystemWorkflowGet) => {
  return async (form: FormData): Promise<Workflow> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const workflow = await ApiClient.Workflow.import(form as any)
      set(draft => {
        const idx = draft.systemWorkflows.findIndex(w => w.id === workflow.id)
        if (idx >= 0) draft.systemWorkflows[idx] = workflow
        else draft.systemWorkflows.push(workflow)
        draft.creating = false
      })
      return workflow
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error =
          error instanceof Error ? error.message : 'Failed to import system workflow'
      })
      throw error
    }
  }
}
