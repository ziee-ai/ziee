import { ApiClient } from '@/api-client'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'
import { mutate } from './_mutate'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async (token: string) => {
  await mutate(set, async () => {
    await ApiClient.RemoteAccess.updateSettings({ ngrok_auth_token: token }, undefined)
    await get().loadStatus()
    emitRemoteAccessStatusChanged('settings')
  })
}
