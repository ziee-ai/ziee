import { ApiClient } from '@/api-client'
import type { TestFireRequest, TestFireResult } from '@/api-client/types'
import type { ScheduledTasksGet, ScheduledTasksSet } from '../state'

export default (_set: ScheduledTasksSet, _get: ScheduledTasksGet) => {
  return async (req: TestFireRequest): Promise<TestFireResult> => {
    return ApiClient.ScheduledTask.testFire(req)
  }
}
