import { createExtensionStore } from '@/modules/chat/core/extensions'
import type { LlmProviderWithModels } from '@/modules/llm-provider/stores/LlmProvider.store'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

/**
 * ModelStore
 * Manages model selection for chat messages
 *
 * This store handles:
 * - Loading user-accessible providers (merged from ChatLlmProvider)
 * - Computing available models from providers
 * - Managing selected model state
 * - Auto-initialization (first available or from conversation)
 * - Event-driven cache invalidation
 */
interface ModelStore {
  // Provider data (merged from ChatLlmProvider)
  /** User-accessible providers from chat endpoint */
  providers: LlmProviderWithModels[]

  /** Loading state for providers */
  loading: boolean

  /** Error message if loading fails */
  error: string | null

  // Model selection
  /** Currently selected model ID (UUID) */
  selectedModelId: string | null

  __init__: {
    __store__?: () => void
    providers?: () => void
  }

  __destroy__?: () => void

  /** Load user-accessible providers (merged from ChatLlmProvider) */
  loadProviders: () => Promise<void>

  /** Set the selected model */
  setModelId: (id: string) => void

  /** Get the selected model ID */
  getModelId: () => string | null

  /** Initialize from conversation's model_id (UUID only) */
  initializeFromConversation: (conversationModelId?: string) => void
}

export const createModelStore = () =>
  createExtensionStore<ModelStore>((set, get) => ({
    // Provider state (merged from ChatLlmProvider)
    providers: [],
    loading: false,
    error: null,

    // Model selection state
    selectedModelId: null,

    __init__: {
      __store__: () => {
        const eventBus = Stores.EventBus
        const GROUP = 'ModelStore'

        // Subscribe to llm_provider.created
        eventBus.on(
          'llm_provider.created',
          async () => {
            await get().loadProviders()
          },
          GROUP,
        )

        // Subscribe to llm_provider.updated
        eventBus.on(
          'llm_provider.updated',
          async event => {
            const { provider } = event.data
            set(state => {
              const existingProvider = state.providers.find(
                p => p.id === provider.id,
              )
              if (!existingProvider) return state

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
          },
          GROUP,
        )

        // Subscribe to llm_provider.deleted
        eventBus.on(
          'llm_provider.deleted',
          async event => {
            const { providerId } = event.data
            set(state => ({
              providers: state.providers.filter(p => p.id !== providerId),
            }))
          },
          GROUP,
        )

        // Subscribe to llm_model.enabled
        eventBus.on(
          'llm_model.enabled',
          async event => {
            const { modelId } = event.data
            set(state => ({
              providers: state.providers.map(p => ({
                ...p,
                llm_models: p.llm_models?.map(m =>
                  m.id === modelId ? { ...m, enabled: true } : m,
                ),
              })),
            }))
          },
          GROUP,
        )

        // Subscribe to llm_model.disabled
        eventBus.on(
          'llm_model.disabled',
          async event => {
            const { modelId } = event.data
            set(state => ({
              providers: state.providers.map(p => ({
                ...p,
                llm_models: p.llm_models?.map(m =>
                  m.id === modelId ? { ...m, enabled: false } : m,
                ),
              })),
            }))
          },
          GROUP,
        )

        // Subscribe to llm_model.deleted
        eventBus.on(
          'llm_model.deleted',
          async event => {
            const { modelId } = event.data
            set(state => ({
              providers: state.providers.map(p => ({
                ...p,
                llm_models: p.llm_models?.filter(m => m.id !== modelId),
              })),
            }))
          },
          GROUP,
        )

        // Subscribe to group-provider assignment changes
        eventBus.on(
          'llm_provider.group_providers_changed',
          async () => {
            await get().loadProviders()
          },
          GROUP,
        )
      },
      providers: () => get().loadProviders(),
    },

    __destroy__: () => {
      Stores.EventBus.removeGroupListeners('ModelStore')
    },

    // Load user-accessible providers (merged from ChatLlmProvider)
    loadProviders: async () => {
      set(state => {
        state.loading = true
        state.error = null
      })
      try {
        const response = await ApiClient.Chat.getUserLlmProviders()
        set(state => {
          state.providers = response.providers
          state.loading = false
        })
      } catch (error: any) {
        console.error('[ModelStore] loadProviders error:', error)
        set(state => {
          state.error = error.message || 'Failed to load providers'
          state.loading = false
        })
      }
    },

    setModelId: (id: string) => {
      set(state => {
        state.selectedModelId = id
      })
    },

    getModelId: () => {
      return get().selectedModelId
    },

    initializeFromConversation: (conversationModelId?: string) => {
      const providers = get().providers

      if (!conversationModelId) {
        // No conversation model - auto-select first enabled model
        for (const provider of providers) {
          if (provider.llm_models && provider.llm_models.length > 0) {
            const firstEnabledModel = provider.llm_models.find(m => m.enabled)
            if (firstEnabledModel) {
              set(state => {
                state.selectedModelId = firstEnabledModel.id
              })
              return
            }
          }
        }
        return
      }

      // Find matching model by ID
      for (const provider of providers) {
        if (provider.llm_models) {
          const matchingModel = provider.llm_models.find(
            model => model.id === conversationModelId && model.enabled,
          )
          if (matchingModel) {
            set(state => {
              state.selectedModelId = matchingModel.id
            })
            return
          }
        }
      }

      // Fallback: conversation model not found or disabled, use first available
      for (const provider of providers) {
        if (provider.llm_models && provider.llm_models.length > 0) {
          const firstEnabledModel = provider.llm_models.find(m => m.enabled)
          if (firstEnabledModel) {
            set(state => {
              state.selectedModelId = firstEnabledModel.id
            })
            return
          }
        }
      }
    },
  }))

/**
 * Augment ChatExtensionStores with ModelStore
 */
declare module '../../types' {
  interface ChatExtensionStores {
    ModelStore: ReturnType<typeof createModelStore>
  }
}
