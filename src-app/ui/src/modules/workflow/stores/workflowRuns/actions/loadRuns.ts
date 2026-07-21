import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { WorkflowRunsGet, WorkflowRunsSet } from '../state'

export default (set: WorkflowRunsSet, _get: WorkflowRunsGet) =>
  async (workflowId: string) => {
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
  }
