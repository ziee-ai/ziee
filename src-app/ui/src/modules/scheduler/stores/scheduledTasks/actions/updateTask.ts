import { ApiClient } from '@/api-client'
import type { ScheduledTask, UpdateScheduledTask } from '@/api-client/types'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (
    id: string,
    patch: UpdateScheduledTask,
  ): Promise<ScheduledTask> => {
    const task = await ApiClient.ScheduledTask.update({ id, ...patch })
    set(draft => {
      const i = draft.tasks.findIndex(t => t.id === id)
      if (i >= 0) draft.tasks[i] = task
    })
    return task
  }
}
