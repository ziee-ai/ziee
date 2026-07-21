import { ApiClient } from '@/api-client'
import { type CreateAuthProviderRequest } from '@/api-client/types'
import type { AuthProvidersAdminGet, AuthProvidersAdminSet } from '../state'
import {
  emitAuthProviderAutoDisabled,
  emitAuthProviderCreated,
} from '@/modules/auth-providers/events'

export default (set: AuthProvidersAdminSet, _get: AuthProvidersAdminGet) =>
  async (req: CreateAuthProviderRequest) => {
    set(s => {
      s.saving = true
      s.error = null
    })
    try {
      const created = await ApiClient.AuthProviders.create(req, undefined)
      set(s => {
        s.saving = false
      })
      try {
        await emitAuthProviderCreated(created.provider)
      } catch (eventError) {
        console.error('Failed to emit auth provider created event:', eventError)
      }
      if (created.connection_warning) {
        try {
          await emitAuthProviderAutoDisabled(created.provider.id, created.connection_warning)
        } catch (eventError) {
          console.error('Failed to emit auth provider auto_disabled event:', eventError)
        }
      }
      return created.provider
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to create provider'
        s.saving = false
      })
      throw e
    }
  }
