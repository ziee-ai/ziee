import { ApiClient } from '@/api-client'
import type { Workflow, WorkflowSet, WorkflowGet } from '../state'

export default (set: WorkflowSet, _get: WorkflowGet) => {
  return async (id: string): Promise<Workflow> => {
    const workflow = await ApiClient.Workflow.get({ id })
    set(draft => {
      const idx = draft.workflows.findIndex(w => w.id === id)
      if (idx >= 0) draft.workflows[idx] = workflow
    })
    return workflow
  }
}
