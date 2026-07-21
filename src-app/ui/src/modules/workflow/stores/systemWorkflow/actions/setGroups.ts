import { ApiClient } from '@/api-client'
import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, _get: SystemWorkflowGet) =>
  async (workflowId: string, groupIds: string[]) => {
    await ApiClient.WorkflowSystem.setGroups({ id: workflowId, group_ids: groupIds })
    set(draft => {
      draft.groups[workflowId] = { groupIds, loading: false }
    })
  }
