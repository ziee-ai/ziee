import { ApiClient } from '@/api-client'
import { type LlmProvider as BaseLlmProvider, type CreateLlmModelRequest, type CreateLlmProviderRequest, type DiscoveredModel, type Group, type LlmModel, type UpdateLlmModelRequest, type UpdateLlmProviderRequest } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import {
  emitGroupLlmProvidersChanged,
  emitLlmModelDeleted,
  emitLlmModelDisabled,
  emitLlmModelEnabled,
  emitLlmProviderCreated,
  emitLlmProviderDeleted,
  emitLlmProviderGroupsChanged,
  emitLlmProviderUpdated,
} from '@/modules/llm-provider/events'
import { sortProviders } from '@/modules/llm-provider/sortProviders'

// Extended type that includes models array.
// TODO: Backend should include llm_models in LlmProvider response.
export interface LlmProviderWithModels extends BaseLlmProvider {
  llm_models?: LlmModel[]
  // Whether an API key is configured (system- or user-level).
  api_key_configured?: boolean
}

export const LlmProviderStoreDef = defineStore('LlmProvider', {
  state: {
    providers: [] as LlmProviderWithModels[],
    isInitialized: false,
    loading: false,
    creating: false,
    updating: false,
    deleting: false,
    llmModelsLoading: {} as Record<string, boolean>, // providerId -> loading
    modelError: {} as Record<string, string>, // providerId -> error message
    llmModelOperations: {} as Record<string, boolean>, // modelId -> operation in progress
    // Model discovery (picker) per provider: results + loading, keyed by providerId.
    discoveredModels: {} as Record<string, DiscoveredModel[]>,
    discoverNotes: {} as Record<string, string[]>,
    discoverLoading: {} as Record<string, boolean>,
    // "Refresh models" (deprecation reconcile) in-flight, keyed by providerId.
    refreshingModels: {} as Record<string, boolean>,
    error: null as string | null,
  },
  actions: (set, get) => {
    const addLlmModelToProvider = (providerId: string, model: LlmModel) => {
      set(state => ({
        providers: state.providers.map(p =>
          p.id === providerId ? { ...p, llm_models: [...(p.llm_models || []), model] } : p,
        ),
      }))
    }
    const updateLlmModelInProvider = (
      providerId: string,
      modelId: string,
      updatedModel: LlmModel,
    ) => {
      set(state => ({
        providers: state.providers.map(p =>
          p.id === providerId
            ? { ...p, llm_models: p.llm_models?.map(m => (m.id === modelId ? updatedModel : m)) }
            : p,
        ),
      }))
    }
    const loadLlmProviders = async (force = false) => {
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
    return {
      loadLlmProviders,
      addLlmModelToProvider,
      updateLlmModelInProvider,
      loadModelsForProvider: async (providerId: string) => {
        // Concurrent-load dedup: skip if a load is already in flight for this
        // provider (SSE handler + user click can race). (audit 05 H-4)
        if (get().llmModelsLoading[providerId]) return
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
          set(state => ({
            providers: state.providers.map(p =>
              p.id === providerId ? { ...p, llm_models: modelsResponse.models } : p,
            ),
            llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
          }))
        } catch (error) {
          const errorMessage = error instanceof Error ? error.message : 'Failed to load models'
          console.error(`Failed to load models for provider ${providerId}:`, error)
          set(state => ({
            llmModelsLoading: { ...state.llmModelsLoading, [providerId]: false },
            modelError: { ...state.modelError, [providerId]: errorMessage },
          }))
        }
      },
      createLlmProvider: async (data: CreateLlmProviderRequest): Promise<LlmProviderWithModels> => {
        if (get().creating) return Promise.resolve(null as any)
        try {
          set({ creating: true, error: null })
          const provider = await ApiClient.LlmProvider.create(data)
          try {
            await emitLlmProviderCreated(provider)
          } catch (eventError) {
            console.error('Failed to emit llm provider created event:', eventError)
          }
          set({ creating: false })
          return { ...provider, llm_models: [] }
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to create provider',
            creating: false,
          })
          throw error
        }
      },
      updateLlmProvider: async (
        id: string,
        data: UpdateLlmProviderRequest,
      ): Promise<LlmProviderWithModels> => {
        const state = get()
        if (state.updating) return Promise.resolve(null as any)
        try {
          set({ updating: true, error: null })
          const provider = await ApiClient.LlmProvider.update({ provider_id: id, ...data })
          try {
            await emitLlmProviderUpdated(provider)
          } catch (eventError) {
            console.error('Failed to emit llm provider updated event:', eventError)
          }
          set({ updating: false })
          const existingProvider = state.providers.find(p => p.id === id)
          return { ...provider, llm_models: existingProvider?.llm_models || [] }
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to update provider',
            updating: false,
          })
          throw error
        }
      },
      deleteLlmProvider: async (id: string) => {
        if (get().deleting) return
        try {
          set({ deleting: true, error: null })
          await ApiClient.LlmProvider.delete({ provider_id: id })
          try {
            await emitLlmProviderDeleted(id)
          } catch (eventError) {
            console.error('Failed to emit llm provider deleted event:', eventError)
          }
          set({ deleting: false })
        } catch (error) {
          set({
            error: error instanceof Error ? error.message : 'Failed to delete provider',
            deleting: false,
          })
          throw error
        }
      },
      clearLlmProviderStoreError: () => {
        set({ error: null })
      },
      findLlmProviderById: (id: string): LlmProviderWithModels | undefined =>
        get().providers.find(p => p.id === id),
      llmProviderHasCredentials: (
        _provider: BaseLlmProvider | LlmProviderWithModels,
      ): boolean => {
        // API key is no longer required to enable a provider (users supply their own).
        return true
      },
      createLlmModel: async (
        providerId: string,
        data: Omit<CreateLlmModelRequest, 'provider_id'>,
      ): Promise<LlmModel> => {
        const model = await ApiClient.LlmModel.create({ ...data, provider_id: providerId })
        // Optimistically append, then refresh so backend enrichment shows.
        addLlmModelToProvider(providerId, model)
        await loadLlmProviders()
        return model
      },
      updateLlmModel: async (modelId: string, data: UpdateLlmModelRequest): Promise<LlmModel> => {
        const updated = await ApiClient.LlmModel.update({ model_id: modelId, ...data })
        const providerId = get().providers.find(p => p.llm_models?.some(m => m.id === modelId))?.id
        if (providerId) updateLlmModelInProvider(providerId, modelId, updated)
        return updated
      },
      enableLlmModel: async (modelId: string): Promise<LlmModel> => {
        try {
          set(state => ({
            llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
            error: null,
          }))
          const model = await ApiClient.LlmModel.update({ model_id: modelId, enabled: true })
          const providerId = get().providers.find(p =>
            p.llm_models?.some(m => m.id === modelId),
          )?.id
          if (providerId) {
            try {
              await emitLlmModelEnabled(modelId, providerId)
            } catch (eventError) {
              console.error('Failed to emit llm model enabled event:', eventError)
            }
          }
          set(state => ({
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
      disableLlmModel: async (modelId: string): Promise<LlmModel> => {
        try {
          set(state => ({
            llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
            error: null,
          }))
          const model = await ApiClient.LlmModel.update({ model_id: modelId, enabled: false })
          const providerId = get().providers.find(p =>
            p.llm_models?.some(m => m.id === modelId),
          )?.id
          if (providerId) {
            try {
              await emitLlmModelDisabled(modelId, providerId)
            } catch (eventError) {
              console.error('Failed to emit llm model disabled event:', eventError)
            }
          }
          set(state => ({
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
      deleteLlmModel: async (modelId: string): Promise<void> => {
        try {
          set(state => ({
            llmModelOperations: { ...state.llmModelOperations, [modelId]: true },
            error: null,
          }))
          const providerId = get().providers.find(p =>
            p.llm_models?.some(m => m.id === modelId),
          )?.id
          await ApiClient.LlmModel.delete({ model_id: modelId })
          if (providerId) {
            try {
              await emitLlmModelDeleted(modelId, providerId)
            } catch (eventError) {
              console.error('Failed to emit llm model deleted event:', eventError)
            }
          }
          set(state => ({
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
      // Discover the models a remote provider offers (catalog + live /v1/models).
      // Populates the picker in the Add Remote Model drawer. Never throws for an
      // empty result — the backend degrades to catalog-only with a note.
      discoverModels: async (providerId: string): Promise<DiscoveredModel[]> => {
        set(state => ({
          discoverLoading: { ...state.discoverLoading, [providerId]: true },
        }))
        try {
          const resp = await ApiClient.LlmProvider.discoverModels({ provider_id: providerId })
          set(state => ({
            discoveredModels: { ...state.discoveredModels, [providerId]: resp.models },
            discoverNotes: { ...state.discoverNotes, [providerId]: resp.notes },
            discoverLoading: { ...state.discoverLoading, [providerId]: false },
          }))
          return resp.models
        } catch (error) {
          set(state => ({
            discoveredModels: { ...state.discoveredModels, [providerId]: [] },
            discoverNotes: {
              ...state.discoverNotes,
              [providerId]: [
                error instanceof Error ? error.message : 'Failed to discover models',
              ],
            },
            discoverLoading: { ...state.discoverLoading, [providerId]: false },
          }))
          return []
        }
      },
      // Refresh a provider's models against its live list: flags deprecated /
      // removed ones and clears the flag on any that reappeared. Replaces the
      // provider's llm_models with the reconciled list.
      refreshProviderModels: async (providerId: string): Promise<LlmModel[]> => {
        set(state => ({
          refreshingModels: { ...state.refreshingModels, [providerId]: true },
        }))
        try {
          const models = await ApiClient.LlmProvider.refreshModels({ provider_id: providerId })
          set(state => ({
            providers: state.providers.map(p =>
              p.id === providerId ? { ...p, llm_models: models } : p,
            ),
            refreshingModels: { ...state.refreshingModels, [providerId]: false },
          }))
          return models
        } catch (error) {
          set(state => ({
            error: error instanceof Error ? error.message : 'Failed to refresh models',
            refreshingModels: { ...state.refreshingModels, [providerId]: false },
          }))
          throw error
        }
      },
      findLlmModelById: (modelId: string): LlmModel | undefined => {
        for (const provider of get().providers) {
          const model = provider.llm_models?.find(m => m.id === modelId)
          if (model) return model
        }
        return undefined
      },
      // Group assignment actions.
      getProvidersForGroup: async (groupId: string): Promise<BaseLlmProvider[]> => {
        try {
          const response = await ApiClient.Group.getProviders({ group_id: groupId })
          // Guard: callers `.map` the result — never hand back undefined.
          return Array.isArray(response.providers) ? response.providers : []
        } catch (error) {
          console.error('Failed to get providers for group:', error)
          throw error
        }
      },
      updateGroupProviders: async (groupId: string, providerIds: string[]) => {
        try {
          await ApiClient.Group.updateProviders({ group_id: groupId, provider_ids: providerIds })
          await emitGroupLlmProvidersChanged(groupId, providerIds)
        } catch (error) {
          console.error('Failed to update group providers:', error)
          throw error
        }
      },
      getGroupsForProvider: async (providerId: string): Promise<Group[]> => {
        try {
          return await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
        } catch (error) {
          console.error('Failed to get groups for provider:', error)
          throw error
        }
      },
      assignGroupToProvider: async (providerId: string, groupId: string) => {
        try {
          await ApiClient.LlmProvider.assignGroup({ provider_id: providerId, group_id: groupId })
          try {
            const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
            await emitLlmProviderGroupsChanged(providerId, groups.map(g => g.id))
          } catch (eventError) {
            console.error('Failed to emit llm provider groups changed event:', eventError)
          }
        } catch (error) {
          console.error('Failed to assign group to provider:', error)
          throw error
        }
      },
      removeGroupFromProvider: async (providerId: string, groupId: string) => {
        try {
          await ApiClient.LlmProvider.removeGroup({ provider_id: providerId, group_id: groupId })
          try {
            const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
            await emitLlmProviderGroupsChanged(providerId, groups.map(g => g.id))
          } catch (eventError) {
            console.error('Failed to emit llm provider groups changed event:', eventError)
          }
        } catch (error) {
          console.error('Failed to remove group from provider:', error)
          throw error
        }
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('llm_provider.created', event => {
      const providerWithModels: LlmProviderWithModels = { ...event.data.provider, llm_models: [] }
      set(state => ({ providers: sortProviders([...state.providers, providerWithModels]) }))
    })
    on('llm_provider.updated', event => {
      const { provider } = event.data
      set(state => {
        const existingProvider = state.providers.find(p => p.id === provider.id)
        const updatedProvider: LlmProviderWithModels = {
          ...provider,
          llm_models: existingProvider?.llm_models || [],
        }
        return {
          providers: sortProviders(
            state.providers.map(p => (p.id === provider.id ? updatedProvider : p)),
          ),
        }
      })
    })
    on('llm_provider.deleted', event => {
      const { providerId } = event.data
      set(state => {
        const { [providerId]: _loading, ...remainingLoading } = state.llmModelsLoading
        const { [providerId]: _error, ...remainingErrors } = state.modelError
        return {
          providers: state.providers.filter(p => p.id !== providerId),
          llmModelsLoading: remainingLoading,
          modelError: remainingErrors,
        }
      })
    })
    on('llm_model.enabled', event => {
      const { modelId } = event.data
      set(state => ({
        providers: state.providers.map(p => ({
          ...p,
          llm_models: p.llm_models?.map(m => (m.id === modelId ? { ...m, enabled: true } : m)),
        })),
      }))
    })
    on('llm_model.disabled', event => {
      const { modelId } = event.data
      set(state => ({
        providers: state.providers.map(p => ({
          ...p,
          llm_models: p.llm_models?.map(m => (m.id === modelId ? { ...m, enabled: false } : m)),
        })),
      }))
    })
    on('llm_model.deleted', event => {
      const { modelId } = event.data
      set(state => ({
        providers: state.providers.map(p => ({
          ...p,
          llm_models: p.llm_models?.filter(m => m.id !== modelId),
        })),
      }))
    })
    // Cross-device sync: the store loads providers WITH models in one pass, so a
    // single forced reload covers both llm_provider + llm_model notifications.
    const reload = () => void actions.loadLlmProviders(true)
    on('sync:llm_provider', reload)
    on('sync:llm_model', reload)
    on('sync:reconnect', reload)
    void actions.loadLlmProviders()
  },
})

export const useLlmProviderStore = LlmProviderStoreDef.store
