import { ApiClient } from '@/api-client'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'
import { mutate } from './_mutate'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async (enabled: boolean) => {
  await mutate(set, async () => {
    await ApiClient.RemoteAccess.updateSettings({ auto_start_tunnel: enabled }, undefined)
    await get().loadStatus()
    emitRemoteAccessStatusChanged('settings')
  })
}
