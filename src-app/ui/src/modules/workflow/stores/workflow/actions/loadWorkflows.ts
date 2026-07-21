import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { WorkflowGet, WorkflowSet } from '../state'

/** Module-level reload coalescing — singleton store, so module scope is safe. */
let pendingReload = false

export default (set: WorkflowSet, get: WorkflowGet) => {
  const doLoad = async () => {
    if (!hasPermissionNow(Permissions.WorkflowsRead)) return
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
        void doLoad()
      }
    }
  }

  return doLoad
}
