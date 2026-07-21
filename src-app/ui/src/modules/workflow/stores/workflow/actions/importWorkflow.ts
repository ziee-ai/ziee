import { ApiClient } from '@/api-client'
import type { Workflow, WorkflowSet } from '../state'

export default (set: WorkflowSet) => {
  return async (form: FormData): Promise<Workflow> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const workflow = await ApiClient.Workflow.import(form as any)
      set(draft => {
        const idx = draft.workflows.findIndex(w => w.id === workflow.id)
        if (idx >= 0) draft.workflows[idx] = workflow
        else draft.workflows.push(workflow)
        draft.creating = false
      })
      return workflow
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error = error instanceof Error ? error.message : 'Failed to import workflow'
      })
      throw error
    }
  }
}
