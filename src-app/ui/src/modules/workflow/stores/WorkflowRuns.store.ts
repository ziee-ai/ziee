import { ApiClient } from '@/api-client'
import { type WorkflowRunSummary } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Per-workflow run history (A4). `runs` is keyed by workflowId so the detail
 * drawer can show the runs of whichever workflow is open. Cross-device updates
 * (run started / finished / deleted on another device) arrive via the
 * `sync:workflow_run` EventBus signal and refetch every loaded workflow.
 */
export const WorkflowRuns = defineStore('WorkflowRuns', {
  immer: true,
  state: {
    runs: {} as Record<string, WorkflowRunSummary[]>,
    loading: {} as Record<string, boolean>,
    deleting: {} as Record<string, boolean>,
  },
  actions: set => ({
    loadRuns: async (workflowId: string) => {
      if (!hasPermissionNow(Permissions.WorkflowsRead)) return
      try {
        set(d => {
          d.loading[workflowId] = true
        })
        const response = await ApiClient.Workflow.listRuns({ id: workflowId })
        set(d => {
          d.runs[workflowId] = response.runs
          d.loading[workflowId] = false
        })
      } catch {
        set(d => {
          d.loading[workflowId] = false
        })
      }
    },
    deleteRun: async (runId: string, workflowId: string) => {
      try {
        set(d => {
          d.deleting[runId] = true
        })
        await ApiClient.Workflow.deleteRun({ run_id: runId })
        set(d => {
          d.deleting[runId] = false
          if (d.runs[workflowId]) {
            d.runs[workflowId] = d.runs[workflowId].filter(r => r.id !== runId)
          }
        })
      } catch (e) {
        set(d => {
          d.deleting[runId] = false
        })
        throw e
      }
    },
  }),
  init: ({ on, get, actions }) => {
    const reload = () => {
      if (!hasPermissionNow(Permissions.WorkflowsRead)) return
      for (const wid of Object.keys(get().runs)) void actions.loadRuns(wid)
    }
    on('sync:workflow_run', reload)
    on('sync:reconnect', reload)
  },
})

export const useWorkflowRunsStore = WorkflowRuns.store
