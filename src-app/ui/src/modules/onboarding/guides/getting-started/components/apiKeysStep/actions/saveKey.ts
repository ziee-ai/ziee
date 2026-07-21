import { message } from '@ziee/kit'
import { ApiClient } from '@/api-client'
import type { ApiKeysStepSet, ApiKeysStepGet } from '../state'
import loadProvidersFactory from './_loadProviders'

export default (set: ApiKeysStepSet, get: ApiKeysStepGet) => {
  const loadProviders = loadProvidersFactory(set, get)
  return async (providerId: string, apiKey: string) => {
    try {
      await ApiClient.LlmProvider.saveUserApiKey(
        { provider_id: providerId, api_key: apiKey },
        undefined,
      )
      await loadProviders()
      message.success('API key saved')
    } catch (error: any) {
      message.error(error.message || 'Failed to save API key')
      throw error
    }
  }
}
