import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import mutate from './_mutate'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async () => {
    await mutate(set, async () => {
      await ApiClient.RemoteAccess.startTunnel(undefined, undefined)
      await get().loadStatus()
    })
  }
}
