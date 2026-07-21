import { ApiClient } from '@/api-client'
import type { GroupSystemWorkflowsWidgetGet, GroupSystemWorkflowsWidgetSet } from '../state'

export default (set: GroupSystemWorkflowsWidgetSet, get: GroupSystemWorkflowsWidgetGet) =>
  async (groupId: string, force = false) => {
    const existing = get().groupWorkflows.get(groupId)
    if (existing?.loading && !force) return
    if (!force && existing?.lastFetched && Date.now() - existing.lastFetched < 30000 && !existing.error) {
      return
    }
    set(state => {
      state.groupWorkflows.set(groupId, {
        groupId,
        workflows: existing?.workflows ?? [],
        loading: true,
        error: null,
        lastFetched: existing?.lastFetched ?? null,
      })
    })
    try {
      const response = await ApiClient.Group.getSystemWorkflows({ group_id: groupId })
      set(state => {
        state.groupWorkflows.set(groupId, {
          groupId,
          workflows: response.workflows,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    } catch (error) {
      console.error(`Failed to load workflows for group ${groupId}:`, error)
      set(state => {
        state.groupWorkflows.set(groupId, {
          groupId,
          workflows: existing?.workflows ?? [],
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load workflows',
          lastFetched: existing?.lastFetched ?? null,
        })
      })
    }
  }
