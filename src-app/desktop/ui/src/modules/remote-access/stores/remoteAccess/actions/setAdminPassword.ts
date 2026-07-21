import { ApiClient } from '@/api-client'
import { emitRemoteAccessStatusChanged } from '@ziee/desktop/modules/remote-access/events/remote-access-events'
import { mutate } from './_mutate'
import type { RemoteAccessGet, RemoteAccessSet } from '../state'

export default (set: RemoteAccessSet, get: RemoteAccessGet) => async (newPassword: string) => {
  await mutate(set, async () => {
    await ApiClient.RemoteAccess.setAdminPassword({ new_password: newPassword }, undefined)
    // The PUT toggles `password_changed_at`; reload so status reflects it.
    await get().loadStatus()
    emitRemoteAccessStatusChanged('settings')
  })
}
