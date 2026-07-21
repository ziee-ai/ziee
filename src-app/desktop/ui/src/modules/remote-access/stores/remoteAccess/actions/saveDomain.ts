import { ApiClient } from '@/api-client'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'
import { mutate } from './_mutate'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async (domain: string | null) => {
  await mutate(set, async () => {
    // null means "clear it"; pass empty string to disambiguate from missing.
    await ApiClient.RemoteAccess.updateSettings({ ngrok_domain: domain ?? '' }, undefined)
    await get().loadStatus()
    emitRemoteAccessStatusChanged('settings')
  })
}
