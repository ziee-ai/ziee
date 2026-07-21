import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { RUNS_PAGE_SIZE } from '../../../components/runTimeline'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

const RUNS_PAGE_SIZE_DEFAULT = RUNS_PAGE_SIZE

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (taskId: string, page = 1, perPage = RUNS_PAGE_SIZE_DEFAULT) => {
    if (!hasPermissionNow(Permissions.SchedulerUse)) return
    set(draft => {
      draft.runsLoading = true
    })
    try {
      let res = await ApiClient.ScheduledTask.listRuns({
        id: taskId,
        page,
        per_page: perPage,
      })
      // Out-of-range page (e.g. a sync reload after retention-prune shrank the
      // history): snap back to page 1 so the user is never stranded on an empty
      // "Showing 0 of N" page with the pager hidden.
      if (res.runs.length === 0 && res.total > 0 && page > 1) {
        res = await ApiClient.ScheduledTask.listRuns({
          id: taskId,
          page: 1,
          per_page: perPage,
        })
      }
      set(draft => {
        draft.runsByTask[taskId] = res.runs
        draft.runsMetaByTask[taskId] = {
          total: res.total,
          page: res.page,
          perPage: res.per_page,
        }
        draft.runsLoading = false
      })
    } catch {
      set(draft => {
        draft.runsLoading = false
      })
    }
  }
}
