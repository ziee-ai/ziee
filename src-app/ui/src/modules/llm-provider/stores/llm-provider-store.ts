import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  LlmProvider as BaseLlmProvider,
  CreateLlmProviderRequest,
  UpdateLlmProviderRequest,
  LlmModel,
  Group,
} from '@/api-client/types'
import { emitGroupLlmProvidersChanged } from '../events'

// Extended type that includes models array
// TODO: Backend should include llm_models in LlmProvider response
export interface LlmProviderWithModels extends BaseLlmProvider {
  llm_models?: LlmModel[]
}

interface LlmProviderState {
  // Data
  providers: LlmProviderWithModels[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // LLM Model loading states
  llmModelsLoading: Record<string, boolean> // providerId -> loading
  modelError: Record<string, string> // providerId -> error message
  llmModelOperations: Record<string, boolean> // modelId -> operation in progress

  // Error state
  error: string | null

  // Actions
  loadLlmProviders: () => Promise<void>
  loadModelsForProvider: (providerId: string) => Promise<void>
  createLlmProvider: (data: CreateLlmProviderRequest) => Promise<LlmProviderWithModels>
  updateLlmProvider: (id: string, data: UpdateLlmProviderRequest) => Promise<LlmProviderWithModels>
  deleteLlmProvider: (id: string) => Promise<void>
  clearLlmProviderStoreError: () => void
  findLlmProviderById: (id: string) => LlmProviderWithModels | undefined
  llmProviderHasCredentials: (provider: BaseLlmProvider | LlmProviderWithModels) => boolean

  // LLM Model actions
  enableLlmModel: (modelId: string) => Promise<LlmModel>
  disableLlmModel: (modelId: string) => Promise<LlmModel>
  deleteLlmModel: (modelId: string) => Promise<void>
  findLlmModelById: (modelId: string) => LlmModel | undefined
  addLlmModelToProvider: (providerId: string, model: LlmModel) => void
  updateLlmModelInProvider: (providerId: string, modelId: string, updatedModel: LlmModel) => void

  // Group assignment actions
  getProvidersForGroup: (groupId: string) => Promise<BaseLlmProvider[]>
  updateGroupProviders: (groupId: string, providerIds: string[]) => Promise<void>

  // Provider group assignment methods
  getGroupsForProvider: (providerId: string) => Promise<Group[]>
  assignGroupToProvider: (providerId: string, groupId: string) => Promise<void>
  removeGroupFromProvider: (providerId: string, groupId: string) => Promise<void>

  __init__: {
    providers: () => Promise<void>
  }
}

export const useLlmProviderStore = create<LlmProviderState>()(
  subscribeWithSelector(
    (set, get): LlmProviderState => ({
      // Initial state
      providers: [],
      isInitialized: false,
      loading: false,
      creating: false,
      updating: false,
      deleting: false,
      llmModelsLoading: {},
      modelError: {},
      llmModelOperations: {},
      error: null,

      // Provider actions
      loadLlmProviders: async () => {
        const state = get()
        if (state.isInitialized || state.loading) {
          return
        }
        try {
          set({ loading: true, error: null })

          const response = await ApiClient.LlmProvider.list({
            page: 1,
            per_page: 50,
          })

          const providers = response.providers

          // Set providers immediately without models
          set({
            providers: providers.map(p => ({ ...p, llm_models: [] })),
            isInitialized: true,
            loading: false,
          })

          // Fetch models for each provider in parallel
          const modelPromises = providers.map(async provider => {
            try {
              const modelsResponse = await ApiClient.LlmModel.list({
                providerId: provider.id,
                page: 1,
                perPage: 100,
              })
              return { providerId: provider.id, models: modelsResponse.models }
            } catch (error) {
              console.error(
                `Failed to load models for provider ${provider.id}:`,
                error,
              )
              return { providerId: provider.id, models: [] }
            }
          })

          const results = await Promise.allSettled(modelPromises)

          // Update each provider with its models
          const providersWithModels = providers.map(provider => {
            const result = results.find(
              r => r.status === 'fulfilled' && r.value.providerId === provider.id,
            )
            const models = result?.status === 'fulfilled' ? result.value.models : []
            return { ...provider, llm_models: models }
          })

          set({
            providers: providersWithModels,
          })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to load providers',
            loading: false,
          })
          throw error
        }
      },

      loadModelsForProvider: async (providerId: string) => {
        try {
          set(state => ({
            llmModelsLoading: { ...state.llmModelsLoading, [providerId]: true },
            modelError: { ...state.modelError, [providerId]: '' },
          }))

          const modelsResponse = await ApiClient.LlmModel.list({
            providerId,
            page: 1,
            perPage: 100,
          })

          // Update provider with fresh models
          set(state => ({
            providers: state.providers.map(p =>
              p.id === providerId ? { ...p, llm_models: modelsResponse.models } : p,
            ),
            llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
          }))
        } catch (error) {
          const errorMessage =
            error instanceof Error ? error.message : 'Failed to load models'
          console.error(`Failed to load models for provider ${providerId}:`, error)
          set(state => ({
            llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
            modelError: { ...state.modelError, [providerId]: errorMessage },
          }))
        }
      },

      createLlmProvider: async (data: CreateLlmProviderRequest) => {
        const state = get()
        if (state.creating) {
          return Promise.resolve(null as any)
        }

        try {
          set({ creating: true, error: null })

          const provider = await ApiClient.LlmProvider.create(data)

          // Add llm_models array to provider
          const providerWithModels: LlmProviderWithModels = {
            ...provider,
            llm_models: [],
          }

          set(state => ({
            providers: [...state.providers, providerWithModels],
            creating: false,
          }))

          return providerWithModels
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to create provider',
            creating: false,
          })
          throw error
        }
      },

      updateLlmProvider: async (id: string, data: UpdateLlmProviderRequest) => {
        const state = get()
        if (state.updating) {
          return Promise.resolve(null as any)
        }

        try {
          set({ updating: true, error: null })

          const provider = await ApiClient.LlmProvider.update({
            provider_id: id,
            ...data,
          })

          // Find existing provider to preserve llm_models
          const existingProvider = state.providers.find(p => p.id === id)
          const updatedProvider: LlmProviderWithModels = {
            ...provider,
            llm_models: existingProvider?.llm_models || [],
          }

          set(state => ({
            providers: state.providers.map(p => (p.id === id ? updatedProvider : p)),
            updating: false,
          }))

          return updatedProvider
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to update provider',
            updating: false,
          })
          throw error
        }
      },

      deleteLlmProvider: async (id: string) => {
        const state = get()
        if (state.deleting) {
          return
        }

        try {
          set({ deleting: true, error: null })

          await ApiClient.LlmProvider.delete({ provider_id: id })

          set(state => {
            // Clean up loading and error states for this provider
            const { [id]: _loading, ...remainingLoading } = state.llmModelsLoading
            const { [id]: _error, ...remainingErrors } = state.modelError

            return {
              providers: state.providers.filter(p => p.id !== id),
              llmModelsLoading: remainingLoading,
              modelError: remainingErrors,
              deleting: false,
            }
          })
        } catch (error) {
          set({
            error:
              error instanceof Error ? error.message : 'Failed to delete provider',
            deleting: false,
          })
          throw error
        }
      },

      clearLlmProviderStoreError: () => {
        set({ error: null })
      },

      findLlmProviderById: (id: string) => {
        return get().providers.find(p => p.id === id)
      },

      llmProviderHasCredentials: (provider: BaseLlmProvider | LlmProviderWithModels) => {
        // Local providers don't need credentials
        if (provider.provider_type === 'local') {
          return true
        }

        // Custom providers might not require API keys
        if (provider.provider_type === 'custom') {
          return true
        }

        // Check if API key exists and is not empty
        return !!(provider.api_key && provider.api_key.trim())
      },

      // LLM Model actions
      enableLlmModel: async (modelId: string) => {
        try {
          set(state => ({
            llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
            error: null,
          }))

          const model = await ApiClient.LlmModel.update({
            model_id: modelId,
            enabled: true,
          })

          // Update the model in the provider's llm_models array
          set(state => ({
            providers: state.providers.map(p => ({
              ...p,
              llm_models: p.llm_models?.map(m => (m.id === modelId ? model : m)),
            })),
            llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
          }))

          return model
        } catch (error) {
          set(state => ({
            error: error instanceof Error ? error.message : 'Failed to enable model',
            llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
          }))
          throw error
        }
      },

      disableLlmModel: async (modelId: string) => {
        try {
          set(state => ({
            llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
            error: null,
          }))

          const model = await ApiClient.LlmModel.update({
            model_id: modelId,
            enabled: false,
          })

          // Update the model in the provider's llm_models array
          set(state => ({
            providers: state.providers.map(p => ({
              ...p,
              llm_models: p.llm_models?.map(m => (m.id === modelId ? model : m)),
            })),
            llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
          }))

          return model
        } catch (error) {
          set(state => ({
            error: error instanceof Error ? error.message : 'Failed to disable model',
            llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
          }))
          throw error
        }
      },

      deleteLlmModel: async (modelId: string) => {
        try {
          set(state => ({
            llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
            error: null,
          }))

          await ApiClient.LlmModel.delete({ model_id: modelId })

          // Remove the model from the provider's llm_models array
          set(state => ({
            providers: state.providers.map(p => ({
              ...p,
              llm_models: p.llm_models?.filter(m => m.id !== modelId),
            })),
            llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
          }))
        } catch (error) {
          set(state => ({
            error: error instanceof Error ? error.message : 'Failed to delete model',
            llmModelOperations: { ...state.llmModelOperations, [modelId]: false },
          }))
          throw error
        }
      },

      findLlmModelById: (modelId: string) => {
        const state = get()
        for (const provider of state.providers) {
          const model = provider.llm_models?.find(m => m.id === modelId)
          if (model) return model
        }
        return undefined
      },

      addLlmModelToProvider: (providerId: string, model: LlmModel) => {
        set(state => ({
          providers: state.providers.map(p => {
            if (p.id === providerId) {
              return {
                ...p,
                llm_models: [...(p.llm_models || []), model],
              }
            }
            return p
          }),
        }))
      },

      updateLlmModelInProvider: (providerId: string, modelId: string, updatedModel: LlmModel) => {
        set(state => ({
          providers: state.providers.map(p => {
            if (p.id === providerId) {
              return {
                ...p,
                llm_models: p.llm_models?.map(m =>
                  m.id === modelId ? updatedModel : m,
                ),
              }
            }
            return p
          }),
        }))
      },

      // Group assignment actions
      getProvidersForGroup: async (groupId: string) => {
        try {
          const response = await ApiClient.Group.getProviders({ group_id: groupId })
          return response.providers
        } catch (error) {
          console.error('Failed to get providers for group:', error)
          throw error
        }
      },

      updateGroupProviders: async (groupId: string, providerIds: string[]) => {
        try {
          await ApiClient.Group.updateProviders({
            group_id: groupId,
            provider_ids: providerIds,
          })
          await emitGroupLlmProvidersChanged(groupId, providerIds)
        } catch (error) {
          console.error('Failed to update group providers:', error)
          throw error
        }
      },

      // Provider group assignment methods
      getGroupsForProvider: async (providerId: string) => {
        try {
          const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
          return groups
        } catch (error) {
          console.error('Failed to get groups for provider:', error)
          throw error
        }
      },

      assignGroupToProvider: async (providerId: string, groupId: string) => {
        try {
          await ApiClient.LlmProvider.assignGroup({
            provider_id: providerId,
            group_id: groupId,
          })
        } catch (error) {
          console.error('Failed to assign group to provider:', error)
          throw error
        }
      },

      removeGroupFromProvider: async (providerId: string, groupId: string) => {
        try {
          await ApiClient.LlmProvider.removeGroup({
            provider_id: providerId,
            group_id: groupId,
          })
        } catch (error) {
          console.error('Failed to remove group from provider:', error)
          throw error
        }
      },

      __init__: {
        providers: () => get().loadLlmProviders(),
      },
    }),
  ),
)
