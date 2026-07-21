import type { SchedulerAdminGet, SchedulerAdminSet } from '../state'
import type { SchedulerAdminSettings, UpdateSchedulerAdminSettings } from '@/api-client/types'
import { updateSettingsFn } from './_applySettings'

export default (set: SchedulerAdminSet, get: SchedulerAdminGet) => {
  const doUpdate = updateSettingsFn(set, get)
  return async (patch: UpdateSchedulerAdminSettings): Promise<SchedulerAdminSettings> => {
    return doUpdate(patch)
  }
}
