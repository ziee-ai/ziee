import { ApiClient } from '@/api-client'
import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, _get: SystemWorkflowGet) =>
  async (workflowId: string) => {
    set(draft => {
      draft.groups[workflowId] = {
        groupIds: draft.groups[workflowId]?.groupIds ?? [],
        loading: true,
      }
    })
    try {
      const groupIds = await ApiClient.WorkflowSystem.getGroups({ id: workflowId })
      set(draft => {
        draft.groups[workflowId] = { groupIds, loading: false }
      })
    } catch (error) {
      set(draft => {
        draft.groups[workflowId] = {
          groupIds: draft.groups[workflowId]?.groupIds ?? [],
          loading: false,
        }
        draft.error =
          error instanceof Error ? error.message : 'Failed to load workflow groups'
      })
    }
  }
