import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type { LlmProviderWithModels } from '@/modules/llm-provider/stores/LlmProvider.store'
import { Stores } from '@/core/stores'

interface ChatLlmProviderState {
  // Data - user-accessible providers from chat endpoint
  providers: LlmProviderWithModels[]

  // Loading state
  loading: boolean
  error: string | null

  // Actions
  loadProviders: () => Promise<void>

  __init__: {
    __store__?: () => void
  }

  __destroy__?: () => void
}

export const useChatLlmProviderStore = create<ChatLlmProviderState>()(
  subscribeWithSelector(
    (set, get): ChatLlmProviderState => ({
      // Initial state
      providers: [],
      loading: false,
      error: null,

      // Load user-accessible providers
      loadProviders: async () => {
        set({ loading: true, error: null })
        try {
          const response = await ApiClient.Chat.getUserLlmProviders()
          set({ providers: response.providers, loading: false })
        } catch (error: any) {
          set({
            error: error.message || 'Failed to load providers',
            loading: false,
          })
        }
      },

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'ChatLlmProviderStore'

          // Load providers when store initializes
          get().loadProviders()

          // Subscribe to llm_provider.created
          eventBus.on('llm_provider.created', async () => {
            // Reload providers to get updated list based on user's group access
            await get().loadProviders()
          }, GROUP)

          // Subscribe to llm_provider.updated
          eventBus.on('llm_provider.updated', async event => {
            const { provider } = event.data
            set(state => {
              const existingProvider = state.providers.find(p => p.id === provider.id)
              if (!existingProvider) return state // User doesn't have access to this provider

              const updatedProvider: LlmProviderWithModels = {
                ...provider,
                llm_models: existingProvider.llm_models || [],
              }
              return {
                providers: state.providers.map(p =>
                  p.id === provider.id ? updatedProvider : p,
                ),
              }
            })
          }, GROUP)

          // Subscribe to llm_provider.deleted
          eventBus.on('llm_provider.deleted', async event => {
            const { providerId } = event.data
            set(state => ({
              providers: state.providers.filter(p => p.id !== providerId),
            }))
          }, GROUP)

          // Subscribe to llm_model.enabled
          eventBus.on('llm_model.enabled', async event => {
            const { modelId } = event.data
            set(state => ({
              providers: state.providers.map(p => ({
                ...p,
                llm_models: p.llm_models?.map(m =>
                  m.id === modelId ? { ...m, enabled: true } : m,
                ),
              })),
            }))
          }, GROUP)

          // Subscribe to llm_model.disabled
          eventBus.on('llm_model.disabled', async event => {
            const { modelId } = event.data
            set(state => ({
              providers: state.providers.map(p => ({
                ...p,
                llm_models: p.llm_models?.map(m =>
                  m.id === modelId ? { ...m, enabled: false } : m,
                ),
              })),
            }))
          }, GROUP)

          // Subscribe to llm_model.deleted
          eventBus.on('llm_model.deleted', async event => {
            const { modelId } = event.data
            set(state => ({
              providers: state.providers.map(p => ({
                ...p,
                llm_models: p.llm_models?.filter(m => m.id !== modelId),
              })),
            }))
          }, GROUP)

          // Subscribe to group-provider assignment changes
          eventBus.on('llm_provider.group_providers_changed', async () => {
            // User's accessible providers may have changed, reload
            await get().loadProviders()
          }, GROUP)
        },
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('ChatLlmProviderStore')
      },
    }),
  ),
)
