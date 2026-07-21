import type { StoreSet } from '@ziee/framework/store-kit'
import type { ScheduledTask, ScheduledTaskRun } from '@/api-client/types'

export interface RunsMeta {
  total: number
  page: number
  perPage: number
}

export const scheduledTasksState = {
  tasks: [] as ScheduledTask[],
  loading: false,
  error: null as string | null,
  runsByTask: {} as Record<string, ScheduledTaskRun[]>,
  runsMetaByTask: {} as Record<string, RunsMeta>,
  runsLoading: false,
}

export type ScheduledTasksState = typeof scheduledTasksState
export type ScheduledTasksSet = StoreSet<ScheduledTasksState>
export type ScheduledTasksGet = () => ScheduledTasksState
