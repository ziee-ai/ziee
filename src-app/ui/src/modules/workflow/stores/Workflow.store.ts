import { ApiClient } from '@/api-client'
import {
  type DryRunResult,
  Permissions,
  type TestRunResponse,
  type ValidateWorkflowResponse,
  type Workflow,
  type WorkflowRunStartResponse,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

/**
 * Workflows store — lists the user's own + accessible system workflows (each
 * carries `scope`), and exposes install / import / validate / dry-run / test /
 * run / delete. Mirrors the Skill store.
 */
// Tracks a reload requested while one is already in flight (singleton store, so
// a module-scoped flag is sufficient and avoids polluting render state).
let pendingReload = false

export const WorkflowStoreDef = defineStore('Workflow', {
  immer: true,
  state: {
    workflows: [] as Workflow[],
    isInitialized: false,
    loading: false,
    creating: false,
    error: null as string | null,
    operationsLoading: {} as Record<string, boolean>,
  },
  actions: (set, get) => {
    const loadWorkflows = async () => {
      if (!hasPermissionNow(Permissions.WorkflowsRead)) return
      // A sync event (or reconnect) can fire while a load is in flight. Remember
      // it and run one more pass after the current load settles rather than
      // dropping it (which would leave the list stale until the next event).
      if (get().loading) {
        pendingReload = true
        return
      }
      try {
        set(draft => {
          draft.loading = true
          draft.error = null
        })
        const response = await ApiClient.Workflow.list({})
        set(draft => {
          draft.workflows = response.workflows
          draft.isInitialized = true
          draft.loading = false
        })
      } catch (error) {
        set(draft => {
          draft.loading = false
          draft.error = error instanceof Error ? error.message : 'Failed to load workflows'
        })
      } finally {
        if (pendingReload) {
          pendingReload = false
          void loadWorkflows()
        }
      }
    }
    return {
      loadWorkflows,
      installFromHub: async (hubId: string): Promise<Workflow> => {
        set(draft => {
          draft.creating = true
          draft.error = null
        })
        try {
          const response = await ApiClient.Hub.createWorkflowFromHub({ hub_id: hubId })
          set(draft => {
            draft.workflows.push(response.workflow)
            draft.creating = false
          })
          return response.workflow
        } catch (error) {
          set(draft => {
            draft.creating = false
            draft.error = error instanceof Error ? error.message : 'Failed to install workflow'
          })
          throw error
        }
      },
      importWorkflow: async (form: FormData): Promise<Workflow> => {
        set(draft => {
          draft.creating = true
          draft.error = null
        })
        try {
          const workflow = await ApiClient.Workflow.import(form as any)
          set(draft => {
            const idx = draft.workflows.findIndex(w => w.id === workflow.id)
            if (idx >= 0) draft.workflows[idx] = workflow
            else draft.workflows.push(workflow)
            draft.creating = false
          })
          return workflow
        } catch (error) {
          set(draft => {
            draft.creating = false
            draft.error = error instanceof Error ? error.message : 'Failed to import workflow'
          })
          throw error
        }
      },
      validateWorkflow: async (yaml: string): Promise<ValidateWorkflowResponse> => {
        return await ApiClient.Workflow.validate({ workflow_yaml: yaml })
      },
      dryRun: async (id: string, inputs: any): Promise<DryRunResult> => {
        return await ApiClient.Workflow.dryRun({ id, inputs })
      },
      test: async (id: string, conversationId?: string): Promise<TestRunResponse> => {
        return await ApiClient.Workflow.test({
          id,
          ...(conversationId ? { conversation_id: conversationId } : {}),
        })
      },
      run: async (
        id: string,
        inputs: any,
        conversationId?: string,
        mocks?: any,
        modelId?: string,
        captureLogs?: boolean,
      ): Promise<WorkflowRunStartResponse> => {
        return await ApiClient.Workflow.run({
          id,
          inputs,
          ...(conversationId ? { conversation_id: conversationId } : {}),
          ...(mocks ? { mocks } : {}),
          ...(modelId ? { model_id: modelId } : {}),
          ...(captureLogs ? { capture_logs: true } : {}),
        })
      },
      deleteWorkflow: async (id: string): Promise<void> => {
        set(draft => {
          draft.operationsLoading[id] = true
          draft.error = null
        })
        try {
          await ApiClient.Workflow.delete({ id })
          set(draft => {
            draft.workflows = draft.workflows.filter(w => w.id !== id)
            delete draft.operationsLoading[id]
          })
        } catch (error) {
          set(draft => {
            delete draft.operationsLoading[id]
            draft.error = error instanceof Error ? error.message : 'Failed to delete workflow'
          })
          throw error
        }
      },
      getWorkflow: async (id: string): Promise<Workflow> => {
        const workflow = await ApiClient.Workflow.get({ id })
        set(draft => {
          const idx = draft.workflows.findIndex(w => w.id === id)
          if (idx >= 0) draft.workflows[idx] = workflow
        })
        return workflow
      },
    }
  },
  init: ({ on, actions }) => {
    const reload = () => void actions.loadWorkflows()
    on('sync:workflow', reload)
    on('sync:reconnect', reload)
    void actions.loadWorkflows()
  },
})

export const useWorkflowStore = WorkflowStoreDef.store
