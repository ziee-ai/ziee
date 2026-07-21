import { ApiClient } from '@/api-client'
import { sortProviders } from '@/modules/llm-provider/sortProviders'
import type { UserLlmProvidersGet, UserLlmProvidersSet } from '../state'

export default (set: UserLlmProvidersSet, _get: UserLlmProvidersGet) =>
  async () => {
    try {
      const [providersRes, keysRes] = await Promise.all([
        ApiClient.LlmProvider.getUserLlmProviders({}, undefined),
        ApiClient.LlmProvider.listUserApiKeys(undefined, undefined),
      ])
      set(state => {
        // Local providers authenticate via an internal proxy token, not a
        // user API key — exclude them from the personal-key list.
        state.providers = sortProviders(
          providersRes.providers.filter(p => p.enabled && p.provider_type !== 'local'),
        )
        state.userKeys = Object.fromEntries(
          keysRes.keys.map(k => [k.provider_id, { masked_key: k.masked_key }]),
        )
        state.loading = false
      })
    } catch (error: any) {
      set(state => {
        state.error = error.message || 'Failed to load providers'
        state.loading = false
      })
    }
  }
