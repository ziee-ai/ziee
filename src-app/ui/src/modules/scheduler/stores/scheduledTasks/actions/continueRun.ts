import { ApiClient } from '@/api-client'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (_set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (runId: string): Promise<string> => {
    const res = await ApiClient.ScheduledTask.continueRun({ run_id: runId })
    return res.conversation_id
  }
}
