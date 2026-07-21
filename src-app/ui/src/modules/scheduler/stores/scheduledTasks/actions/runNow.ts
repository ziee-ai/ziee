import { ApiClient } from '@/api-client'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (_set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (id: string) => {
    await ApiClient.ScheduledTask.runNow({ id })
  }
}
