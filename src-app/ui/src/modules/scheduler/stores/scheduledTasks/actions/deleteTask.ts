import { ApiClient } from '@/api-client'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (id: string) => {
    await ApiClient.ScheduledTask.delete({ id })
    set(draft => {
      draft.tasks = draft.tasks.filter(t => t.id !== id)
    })
  }
}
