import { ApiClient } from '@/api-client'
import type { CreateScheduledTask, ScheduledTask } from '@/api-client/types'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (body: CreateScheduledTask): Promise<ScheduledTask> => {
    const task = await ApiClient.ScheduledTask.create(body)
    set(draft => {
      draft.tasks.unshift(task)
    })
    return task
  }
}
