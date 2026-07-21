import { ApiClient } from '@/api-client'
import type { ApiKeysStepSet, ApiKeysStepGet } from '../state'

export default (set: ApiKeysStepSet, _get: ApiKeysStepGet) =>
  async () => {
    set(s => {
      s.loadingProviders = true
      s.providersError = null
    })
    try {
      const [providersRes, keysRes] = await Promise.all([
        ApiClient.LlmProvider.getUserLlmProviders({}, undefined),
        ApiClient.LlmProvider.listUserApiKeys(undefined, undefined),
      ])
      set(s => {
        // Local providers authenticate via an internal proxy token, not a
        // user API key — exclude them from the key-entry list.
        s.providers = providersRes.providers.filter(
          p => p.enabled && p.provider_type !== 'local',
        )
        s.userKeys = Object.fromEntries(
          keysRes.keys.map(k => [k.provider_id, { masked_key: k.masked_key }]),
        )
        s.loadingProviders = false
      })
    } catch (error: any) {
      console.error('[ApiKeysStep] loadProviders error:', error)
      set(s => {
        s.providersError = error.message || 'Failed to load providers'
        s.loadingProviders = false
      })
    }
  }
