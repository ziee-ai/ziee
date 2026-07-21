import { ApiClient } from '@/api-client'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (_set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (
    taskId: string,
    limit: number,
  ): Promise<string> => {
    const res = await ApiClient.ScheduledTask.continueSeries({
      id: taskId,
      limit,
    })
    return res.conversation_id
  }
}
