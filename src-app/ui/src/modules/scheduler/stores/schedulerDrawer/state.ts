import type { ScheduledTask } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const schedulerDrawerState = {
  open: false,
  editing: null as ScheduledTask | null,
  loading: false,
}

export type SchedulerDrawerState = typeof schedulerDrawerState
export type SchedulerDrawerSet = StoreSet<SchedulerDrawerState>
export type SchedulerDrawerGet = () => SchedulerDrawerState
