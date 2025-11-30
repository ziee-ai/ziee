import { createExtensionStore } from '../../core/extensions'
import type { LlmModel } from '@/api-client/types'
import type { LlmProviderWithModels } from '@/modules/llm-provider/stores/LlmProvider.store'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'

/**
 * Model option for select dropdown
 */
export interface ModelOption {
  label: string
  value: string  // Format: "providerId:modelId"
  description?: string
}

/**
 * Model group (provider with models)
 */
export interface ModelGroup {
  label: string  // Provider name
  options: ModelOption[]
}

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
  /** Currently selected model in format "providerId:modelId" */
  selectedModelId: string | null

  /** Available models grouped by provider (computed from providers) */
  get availableModels(): ModelGroup[]

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

    // Computed getter for available models
    get availableModels(): ModelGroup[] {
      const providers = get().providers
      const modelGroups: ModelGroup[] = []

      providers.forEach((provider: LlmProviderWithModels) => {
        if (provider.llm_models && provider.llm_models.length > 0) {
          const enabledModels = provider.llm_models.filter(
            (model: LlmModel) => model.enabled,
          )

          if (enabledModels.length > 0) {
            modelGroups.push({
              label: provider.name,
              options: enabledModels.map((model: LlmModel) => ({
                label: model.display_name || model.name,
                value: `${provider.id}:${model.id}`,
                description: model.description,
              })),
            })
          }
        }
      })

      return modelGroups
    },

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
      const availableModels = get().availableModels

      if (!conversationModelId) {
        // No conversation model - auto-select first available
        if (availableModels.length > 0 && availableModels[0].options.length > 0) {
          set(state => {
            state.selectedModelId = availableModels[0].options[0].value
          })
        }
        return
      }

      // Find matching model in format "providerId:modelId"
      for (const providerGroup of availableModels) {
        const matchingModel = providerGroup.options.find(model =>
          model.value.endsWith(`:${conversationModelId}`),
        )
        if (matchingModel) {
          set(state => {
            state.selectedModelId = matchingModel.value
          })
          return
        }
      }

      // Fallback: conversation model not found, use first available
      if (availableModels.length > 0 && availableModels[0].options.length > 0) {
        set(state => {
          state.selectedModelId = availableModels[0].options[0].value
        })
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
