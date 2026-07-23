import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async () => {
    if (!hasPermissionNow(Permissions.SchedulerUse)) return
    set(draft => {
      draft.loading = true
      draft.error = null
    })
    try {
      const tasks = await ApiClient.ScheduledTask.list({})
      set(draft => {
        draft.tasks = tasks
        draft.loading = false
      })
    } catch (error) {
      set(draft => {
        draft.loading = false
        draft.error =
          error instanceof Error
            ? error.message
            : 'Failed to load scheduled tasks'
      })
    }
  }
}
