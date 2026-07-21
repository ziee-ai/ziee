import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { UserProviderKeysGet, UserProviderKeysSet } from '../state'
import doLoadKeysFactory from './_doLoadKeys'

export default (set: UserProviderKeysSet, get: UserProviderKeysGet) => {
  const doLoadKeys = doLoadKeysFactory(set, get)
  return async () => {
    // `sync:reconnect` fires for every store regardless of audience; skip the
    // refetch for users without `profile::read` (the endpoint would 403).
    if (!hasPermissionNow(Permissions.ProfileRead)) return
    if (get().initialized) return
    await doLoadKeys()
    set({ initialized: true })
  }
}
