import { ApiClient } from '@/api-client'
import type { ScheduledTask } from '@/api-client/types'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (id: string, enabled: boolean): Promise<ScheduledTask> => {
    const task = await ApiClient.ScheduledTask.update({ id, enabled })
    set(draft => {
      const i = draft.tasks.findIndex(t => t.id === id)
      if (i >= 0) draft.tasks[i] = task
    })
    return task
  }
}
