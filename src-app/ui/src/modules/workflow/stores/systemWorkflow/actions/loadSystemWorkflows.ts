import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { SystemWorkflowGet, SystemWorkflowSet } from '../state'

export default (set: SystemWorkflowSet, get: SystemWorkflowGet) =>
  async () => {
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
  }
