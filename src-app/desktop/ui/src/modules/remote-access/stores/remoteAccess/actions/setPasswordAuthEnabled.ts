import { ApiClient } from '@/api-client'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'
import { mutate } from './_mutate'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async (enabled: boolean) => {
  await mutate(set, async () => {
    await ApiClient.RemoteAccess.updateSettings({ password_auth_enabled: enabled }, undefined)
    await get().loadStatus()
    emitRemoteAccessStatusChanged('settings')
  })
}
