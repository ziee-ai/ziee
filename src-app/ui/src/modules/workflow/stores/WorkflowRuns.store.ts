import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { Permissions, type WorkflowRunSummary } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

/**
 * Per-workflow run history (A4). `runs` is keyed by workflowId so the detail
 * drawer can show the runs of whichever workflow is open. Cross-device updates
 * (run started / finished / deleted on another device) arrive via the
 * `sync:workflow_run` EventBus signal and refetch every loaded workflow.
 */
interface WorkflowRunsState {
  runs: Record<string, WorkflowRunSummary[]>
  loading: Record<string, boolean>
  deleting: Record<string, boolean>

  __init__: {
    __store__?: () => void
  }

  loadRuns: (workflowId: string) => Promise<void>
  deleteRun: (runId: string, workflowId: string) => Promise<void>
}

export const useWorkflowRunsStore = create<WorkflowRunsState>()(
  subscribeWithSelector(
    immer(
      (set, get): WorkflowRunsState => ({
        runs: {},
        loading: {},
        deleting: {},

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'WorkflowRunsStore'
            const reload = () => {
              if (!hasPermissionNow(Permissions.WorkflowsRead)) return
              for (const wid of Object.keys(get().runs)) {
                void get().loadRuns(wid)
              }
            }
            eventBus.on('sync:workflow_run', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
        },

        loadRuns: async (workflowId: string) => {
          if (!hasPermissionNow(Permissions.WorkflowsRead)) return
          try {
            set(draft => {
              draft.loading[workflowId] = true
            })
            const response = await ApiClient.Workflow.listRuns({ id: workflowId })
            set(draft => {
              draft.runs[workflowId] = response.runs
              draft.loading[workflowId] = false
            })
          } catch {
            set(draft => {
              draft.loading[workflowId] = false
            })
          }
        },

        deleteRun: async (runId: string, workflowId: string) => {
          try {
            set(draft => {
              draft.deleting[runId] = true
            })
            await ApiClient.Workflow.deleteRun({ run_id: runId })
            set(draft => {
              draft.deleting[runId] = false
              if (draft.runs[workflowId]) {
                draft.runs[workflowId] = draft.runs[workflowId].filter(
                  r => r.id !== runId,
                )
              }
            })
          } catch (e) {
            set(draft => {
              draft.deleting[runId] = false
            })
            throw e
          }
        },
      }),
    ),
  ),
)
