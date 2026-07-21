import { ApiClient } from '@/api-client'
import type { Workflow } from '@/api-client/types'
import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, _get: SystemWorkflowGet) => {
  return async (hubId: string, groups?: string[]): Promise<Workflow> => {
    set(draft => {
      draft.creating = true
      draft.error = null
    })
    try {
      const response = await ApiClient.Hub.createSystemWorkflowFromHub({
        hub_id: hubId,
        ...(groups && groups.length > 0 ? { groups } : {}),
      })
      set(draft => {
        draft.systemWorkflows.push(response.workflow)
        draft.creating = false
      })
      return response.workflow
    } catch (error) {
      set(draft => {
        draft.creating = false
        draft.error =
          error instanceof Error ? error.message : 'Failed to install system workflow'
      })
      throw error
    }
  }
}
