import type { Workflow } from '@/api-client/types'
import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'

interface GroupWorkflows {
  groupId: string
  workflows: Workflow[]
  loading: boolean
  error: string | null
  lastFetched: number | null
}

/** Group → assigned system workflows (single-call, 30s cache; no event subs). */
export const GroupSystemWorkflowsWidget = defineStore('GroupSystemWorkflowsWidget', {
  immer: true,
  state: { groupWorkflows: new Map<string, GroupWorkflows>() },
  actions: (set, get) => ({
    loadWorkflowsForGroup: async (groupId: string, force = false) => {
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
    },
    updateGroupWorkflows: async (groupId: string, workflowIds: string[]) => {
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
    },
  }),
})

export const useGroupSystemWorkflowsWidgetStore = GroupSystemWorkflowsWidget.store
