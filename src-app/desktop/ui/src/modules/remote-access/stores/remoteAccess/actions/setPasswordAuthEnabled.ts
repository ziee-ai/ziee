import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import mutate from './_mutate'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async (enabled: boolean) => {
    await mutate(set, async () => {
      await ApiClient.RemoteAccess.updateSettings({ password_auth_enabled: enabled }, undefined)
      await get().loadStatus()
    })
  }
}
