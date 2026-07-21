import type { StoreSet } from '@ziee/framework/store-kit'
import type { SchedulerAdminSettings } from '@/api-client/types'

/** Deployment-wide scheduler admin settings (quota / cadence floor / retention). */
export const schedulerAdminState = {
  settings: null as SchedulerAdminSettings | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type SchedulerAdminState = typeof schedulerAdminState
export type SchedulerAdminSet = StoreSet<SchedulerAdminState>
export type SchedulerAdminGet = () => SchedulerAdminState
