import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { SchedulerAdminGet, SchedulerAdminSet } from '../state'
import { loadSettingsFn } from './_applySettings'

export default (set: SchedulerAdminSet, get: SchedulerAdminGet) => {
  const doLoad = loadSettingsFn(set, get)
  return async () => {
    if (!hasPermissionNow(Permissions.SchedulerAdminRead)) return
    await doLoad()
  }
}
