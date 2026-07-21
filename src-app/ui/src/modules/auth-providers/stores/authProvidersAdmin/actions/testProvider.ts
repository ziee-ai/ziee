import { ApiClient } from '@/api-client'
import { type TestProviderResponse } from '@/api-client/types'
import type { AuthProvidersAdminGet, AuthProvidersAdminSet } from '../state'
import loadProvidersFactory from './loadProviders'
import {
  emitAuthProviderAutoDisabled,
} from '@/modules/auth-providers/events'

export default (set: AuthProvidersAdminSet, get: AuthProvidersAdminGet) => {
  const loadProviders = loadProvidersFactory(set)
  return async (id: string): Promise<TestProviderResponse> => {
    set(s => {
      s.testingIds.add(id)
    })
    try {
      // Snapshot enabled BEFORE the test; the server may flip it.
      const wasEnabled = get().providers.find(p => p.id === id)?.enabled === true
      const res = await ApiClient.AuthProviders.test({ id }, undefined)
      // The originator's SSE self-echo guard suppresses the server's
      // sync_publish, so refresh inline to surface last_test_* + enabled.
      await loadProviders()
      set(s => {
        s.testingIds.delete(id)
      })
      if (wasEnabled && !res.ok) {
        try {
          await emitAuthProviderAutoDisabled(id, res.message)
        } catch (eventError) {
          console.error('Failed to emit auth provider auto_disabled event:', eventError)
        }
      }
      return res
    } catch (e: any) {
      set(s => {
        s.testingIds.delete(id)
      })
      return { ok: false, message: e?.message ?? 'Test failed' }
    }
  }
}
