import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { CitationsGet, CitationsSet } from '../state'
import loadEntriesFactory from './_loadEntries'

export default (set: CitationsSet, get: CitationsGet) => {
  const loadEntries = loadEntriesFactory(set, get)
  return async (projectId?: string | null) => {
    // `sync:reconnect` fires for every store regardless of audience; skip the
    // refetch for users without `citations::use` (the endpoint would 403).
    if (!hasPermissionNow(Permissions.CitationsUse)) return
    return loadEntries(projectId)
  }
}
