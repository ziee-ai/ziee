import { ApiClient } from '@/api-client'
import { type AuthProviderResponse, type UpdateAuthProviderRequest } from '@/api-client/types'
import type { AuthProvidersAdminSet } from '../state'
import {
  emitAuthProviderAutoDisabled,
  emitAuthProviderUpdated,
} from '@/modules/auth-providers/events'

export default (set: AuthProvidersAdminSet) =>
  async (id: string, req: UpdateAuthProviderRequest): Promise<AuthProviderResponse> => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const updated = await ApiClient.AuthProviders.update({ id, ...req }, undefined)
      set(s => {
        s.saving = false
      })
      try {
        await emitAuthProviderUpdated(updated)
      } catch (eventError) {
        console.error('Failed to emit auth provider updated event:', eventError)
      }
      return updated
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to update provider'
        s.saving = false
      })
      // Backend returns 400 AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK when an
      // enable-transition probe fails. Match the stable error_code (not a
      // message substring / `req.enabled`, which a dup-name 400 would
      // false-trip). The row is reverted server-side; the listener reloads.
      const code = (e as { error_code?: string })?.error_code
      if (code === 'AUTH_PROVIDER_ENABLE_FAILED_HEALTH_CHECK') {
        try {
          await emitAuthProviderAutoDisabled(
            id,
            typeof e?.message === 'string' ? e.message : 'Probe failed',
          )
        } catch (eventError) {
          console.error('Failed to emit auth provider auto_disabled event:', eventError)
        }
      }
      throw e
    }
  }
