import { ApiClient } from '@/api-client'
import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, _get: SystemWorkflowGet) =>
  async (id: string) => {
    await ApiClient.Workflow.deleteSystem({ id })
    set(draft => {
      draft.systemWorkflows = draft.systemWorkflows.filter(w => w.id !== id)
    })
  }
