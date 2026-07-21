import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import mutate from './_mutate'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async (domain: string | null) => {
    await mutate(set, async () => {
      // null means "clear it"; pass empty string to disambiguate from missing.
      await ApiClient.RemoteAccess.updateSettings({ ngrok_domain: domain ?? '' }, undefined)
      await get().loadStatus()
    })
  }
}
