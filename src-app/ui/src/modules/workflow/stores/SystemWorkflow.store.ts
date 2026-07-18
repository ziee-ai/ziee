import { ApiClient } from '@/api-client'
import { Permissions, type Workflow } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Admin store for system-scope workflows. list / get / delete for system
 * workflows; system install (with optional group restriction) goes through the
 * Hub namespace (`createSystemWorkflowFromHub`). Local import reuses the shared
 * `Workflow.import` with `scope=system`.
 */
export const SystemWorkflow = defineStore('SystemWorkflow', {
  immer: true,
  state: {
    systemWorkflows: [] as Workflow[],
    isInitialized: false,
    loading: false,
    creating: false,
    error: null as string | null,
    // Per-workflow assigned group ids (lazy-loaded by the assignment card).
    groups: {} as Record<string, { groupIds: string[]; loading: boolean }>,
  },
  actions: (set, get) => ({
    loadSystemWorkflows: async () => {
      if (!hasPermissionNow(Permissions.WorkflowsManageSystem)) return
      if (get().loading) return
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
        })
        const response = await ApiClient.Workflow.listSystem()
        set(draft => {
          draft.systemWorkflows = response.workflows
          draft.isInitialized = true
          draft.loading = false
        })
      } catch (error) {
        set(draft => {
          draft.loading = false
          draft.error =
            error instanceof Error ? error.message : 'Failed to load system workflows'
        })
      }
    },
    installSystemFromHub: async (hubId: string, groups?: string[]): Promise<Workflow> => {
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
    },
    importSystemWorkflow: async (form: FormData): Promise<Workflow> => {
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
    },
    deleteSystemWorkflow: async (id: string): Promise<void> => {
      await ApiClient.Workflow.deleteSystem({ id })
      set(draft => {
        draft.systemWorkflows = draft.systemWorkflows.filter(w => w.id !== id)
      })
    },
    loadGroups: async (workflowId: string) => {
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
    },
    setGroups: async (workflowId: string, groupIds: string[]) => {
      await ApiClient.WorkflowSystem.setGroups({ id: workflowId, group_ids: groupIds })
      set(draft => {
        draft.groups[workflowId] = { groupIds, loading: false }
      })
    },
  }),
  init: ({ on, actions }) => {
    const reload = () => void actions.loadSystemWorkflows()
    on('sync:workflow_system', reload)
    on('sync:reconnect', reload)
    void actions.loadSystemWorkflows()
  },
})

export const useSystemWorkflowStore = SystemWorkflow.store
