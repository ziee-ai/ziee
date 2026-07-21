import { ApiClient } from '@/api-client'
import type { GroupSystemWorkflowsWidgetGet, GroupSystemWorkflowsWidgetSet } from '../state'

export default (set: GroupSystemWorkflowsWidgetSet, _get: GroupSystemWorkflowsWidgetGet) =>
  async (groupId: string, workflowIds: string[]) => {
    const response = await ApiClient.Group.updateSystemWorkflows({
      group_id: groupId,
      workflow_ids: workflowIds,
    })
    set(state => {
      state.groupWorkflows.set(groupId, {
        groupId,
        workflows: response.workflows,
        loading: false,
        error: null,
        lastFetched: Date.now(),
      })
    })
  }
