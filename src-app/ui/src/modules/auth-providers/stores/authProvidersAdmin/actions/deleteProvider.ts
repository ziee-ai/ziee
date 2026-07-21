import { ApiClient } from '@/api-client'
import type { AuthProvidersAdminGet, AuthProvidersAdminSet } from '../state'
import { emitAuthProviderDeleted } from '@/modules/auth-providers/events'

export default (set: AuthProvidersAdminSet, _get: AuthProvidersAdminGet) =>
  async (id: string) => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const res = await ApiClient.AuthProviders.delete({ id }, undefined)
      set(s => {
        s.saving = false
      })
      try {
        await emitAuthProviderDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit auth provider deleted event:', eventError)
      }
      return { affected_user_links: res.affected_user_links }
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to delete provider'
        s.saving = false
      })
      throw e
    }
  }
