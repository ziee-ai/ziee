import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import mutate from './_mutate'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async (token: string) => {
    await mutate(set, async () => {
      await ApiClient.RemoteAccess.updateSettings({ ngrok_auth_token: token }, undefined)
      await get().loadStatus()
    })
  }
}
