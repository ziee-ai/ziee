import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { Permissions, type Workflow } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * Admin store for system-scope workflows. The generated client exposes
 * list / get / delete for system workflows; system install (with
 * optional group restriction) goes through the Hub namespace
 * (`createSystemWorkflowFromHub`). Local import reuses the shared
 * `Workflow.import` with `scope=system`.
 */
interface SystemWorkflowState {
  systemWorkflows: Workflow[]
  isInitialized: boolean
  loading: boolean
  creating: boolean
  error: string | null

  __init__: {
    __store__?: () => void
    systemWorkflows: () => Promise<void>
  }
  __destroy__?: () => void

  loadSystemWorkflows: () => Promise<void>
  installSystemFromHub: (hubId: string, groups?: string[]) => Promise<Workflow>
  importSystemWorkflow: (form: FormData) => Promise<Workflow>
  deleteSystemWorkflow: (id: string) => Promise<void>
}

export const useSystemWorkflowStore = create<SystemWorkflowState>()(
  subscribeWithSelector(
    immer(
      (set, get): SystemWorkflowState => ({
        systemWorkflows: [],
        isInitialized: false,
        loading: false,
        creating: false,
        error: null,

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'SystemWorkflowStore'
            const reload = () => void get().loadSystemWorkflows()
            eventBus.on('sync:workflow_system', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
          systemWorkflows: () => get().loadSystemWorkflows(),
        },

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
                error instanceof Error
                  ? error.message
                  : 'Failed to load system workflows'
            })
          }
        },

        installSystemFromHub: async (
          hubId: string,
          groups?: string[],
        ): Promise<Workflow> => {
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
                error instanceof Error
                  ? error.message
                  : 'Failed to install system workflow'
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
              const idx = draft.systemWorkflows.findIndex(
                w => w.id === workflow.id,
              )
              if (idx >= 0) draft.systemWorkflows[idx] = workflow
              else draft.systemWorkflows.push(workflow)
              draft.creating = false
            })
            return workflow
          } catch (error) {
            set(draft => {
              draft.creating = false
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to import system workflow'
            })
            throw error
          }
        },

        deleteSystemWorkflow: async (id: string): Promise<void> => {
          await ApiClient.Workflow.deleteSystem({ id })
          set(draft => {
            draft.systemWorkflows = draft.systemWorkflows.filter(
              w => w.id !== id,
            )
          })
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('SystemWorkflowStore')
        },
      }),
    ),
  ),
)
