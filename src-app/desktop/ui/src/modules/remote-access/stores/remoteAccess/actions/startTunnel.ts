import { ApiClient } from '@/api-client'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'
import { mutate } from './_mutate'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async () => {
  await mutate(set, async () => {
    await ApiClient.RemoteAccess.startTunnel(undefined, undefined)
    await get().loadStatus()
    emitRemoteAccessStatusChanged('tunnel')
  })
}
