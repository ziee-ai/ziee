import { ApiClient } from '@/api-client'
import type { RemoteAccessSet, RemoteAccessGet } from '../state'
import mutate from './_mutate'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => {
  return async (newPassword: string) => {
    await mutate(set, async () => {
      await ApiClient.RemoteAccess.setAdminPassword({ new_password: newPassword }, undefined)
      // The PUT toggles `password_changed_at`; reload so status reflects it.
      await get().loadStatus()
    })
  }
}
