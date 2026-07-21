import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { sortProviders } from '@/modules/llm-provider/sortProviders'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (force = false) => {
    // Loads providers AND each provider's models — gate on BOTH reads so a
    // sub-admin holding only one perm doesn't 403 on the other during resync.
    if (!hasPermissionNow({ allOf: [Permissions.LlmProvidersRead, Permissions.LlmModelsRead] })) {
      return
    }
    const state = get()
    if ((state.isInitialized && !force) || state.loading) return
    try {
      set({ loading: true, error: null })
      const response = await ApiClient.LlmProvider.list({ page: 1, per_page: 50 })
      const providers = sortProviders(response.providers)
      // Set providers immediately without models.
      set({
        providers: providers.map(p => ({ ...p, llm_models: [] })),
        isInitialized: true,
        loading: false,
      })
      // Fetch models for each provider in parallel.
      const modelPromises = providers.map(async provider => {
        try {
          const modelsResponse = await ApiClient.LlmModel.list({
            providerId: provider.id,
            page: 1,
            perPage: 100,
          })
          return { providerId: provider.id, models: modelsResponse.models }
        } catch (error) {
          console.error(`Failed to load models for provider ${provider.id}:`, error)
          return { providerId: provider.id, models: [] }
        }
      })
      const results = await Promise.allSettled(modelPromises)
      const providersWithModels = providers.map(provider => {
        const result = results.find(
          r => r.status === 'fulfilled' && r.value.providerId === provider.id,
        )
        const models = result?.status === 'fulfilled' ? result.value.models : []
        return { ...provider, llm_models: models }
      })
      set({ providers: providersWithModels })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load providers',
        loading: false,
      })
      throw error
    }
  }
