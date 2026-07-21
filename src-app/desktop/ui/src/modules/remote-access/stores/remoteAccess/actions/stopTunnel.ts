import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import mutate from './_mutate'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async () => {
    await mutate(set, async () => {
      await ApiClient.RemoteAccess.stopTunnel(undefined, undefined)
      get().stopMagicLinkRotation()
      set((s) => {
        s.magicLink = null
      })
      await get().loadStatus()
    })
  }
}
