import { ApiClient } from '@/api-client'
import { Permissions, type ProviderWithModels } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { sortProviders } from '@/modules/llm-provider/sortProviders'

/**
 * ModelPicker store — the chat composer's model selection plus the cached list
 * of user-accessible providers/models. Lives at `Stores.ModelPicker`. `providers`
 * lazy-loads once; listeners keep it in sync with admin llm_provider/llm_model
 * mutations; `selectedModelId` is scoped per conversation via the chat
 * extension's `initialize()` → `initializeFromConversation`.
 */
export const ModelPicker = defineStore('ModelPicker', {
  immer: true,
  state: {
    /** User-accessible providers from the chat endpoint. */
    providers: [] as ProviderWithModels[],
    loading: false,
    error: null as string | null,
    /** Currently selected model ID (UUID). */
    selectedModelId: null as string | null,
  },
  actions: (set, get) => {
    const initializeFromConversation = (conversationModelId?: string) => {
      const providers = get().providers
      const selectFirstEnabled = (): boolean => {
        for (const provider of providers) {
          if (provider.llm_models && provider.llm_models.length > 0) {
            const firstEnabledModel = provider.llm_models.find(m => m.enabled)
            if (firstEnabledModel) {
              set(state => {
                state.selectedModelId = firstEnabledModel.id
              })
              return true
            }
          }
        }
        return false
      }
      if (!conversationModelId) {
        // No conversation model — auto-select first enabled model.
        selectFirstEnabled()
        return
      }
      // Find matching enabled model by ID.
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
      // Fallback: conversation model not found or disabled → first available.
      selectFirstEnabled()
    }
    const loadProviders = async () => {
      // Permission-gate the shell-eager-load fetch — the chat picker accesses
      // this on every chat render; the endpoint is gated on user_llm_providers::read.
      if (!hasPermissionNow(Permissions.UserLlmProvidersRead)) return
      set(state => {
        state.loading = true
        state.error = null
      })
      try {
        const response = await ApiClient.LlmProvider.getUserLlmProviders({}, undefined)
        set(state => {
          state.providers = sortProviders(response.providers)
          state.loading = false
        })
        // Auto-select first model if none is selected yet.
        if (!get().selectedModelId) initializeFromConversation()
      } catch (error: any) {
        console.error('[ModelPicker] loadProviders error:', error)
        set(state => {
          state.error = error.message || 'Failed to load providers'
          state.loading = false
        })
      }
    }
    return {
      loadProviders,
      initializeFromConversation,
      setModelId: (id: string) => {
        set(state => {
          state.selectedModelId = id
        })
      },
      getModelId: (): string | null => get().selectedModelId,
    }
  },
  init: ({ on, set, actions }) => {
    on('llm_provider.created', () => void actions.loadProviders())
    on('llm_provider.updated', event => {
      const { provider } = event.data
      set(state => {
        const existingProvider = state.providers.find(p => p.id === provider.id)
        if (!existingProvider) return
        const updatedProvider: ProviderWithModels = {
          ...existingProvider,
          ...provider,
          llm_models: existingProvider.llm_models || [],
          api_key_configured: existingProvider.api_key_configured,
        }
        state.providers = state.providers.map(p =>
          p.id === provider.id ? updatedProvider : p,
        )
      })
    })
    on('llm_provider.deleted', event => {
      set(state => {
        state.providers = state.providers.filter(p => p.id !== event.data.providerId)
      })
    })
    on('llm_model.enabled', event => {
      const { modelId } = event.data
      set(state => {
        state.providers = state.providers.map(p => ({
          ...p,
          llm_models: p.llm_models?.map(m => (m.id === modelId ? { ...m, enabled: true } : m)),
        }))
      })
    })
    on('llm_model.disabled', event => {
      const { modelId } = event.data
      set(state => {
        state.providers = state.providers.map(p => ({
          ...p,
          llm_models: p.llm_models?.map(m => (m.id === modelId ? { ...m, enabled: false } : m)),
        }))
      })
    })
    on('llm_model.deleted', event => {
      const { modelId } = event.data
      set(state => {
        state.providers = state.providers.map(p => ({
          ...p,
          llm_models: p.llm_models?.filter(m => m.id !== modelId),
        }))
      })
    })
    on('llm_provider.group_providers_changed', () => void actions.loadProviders())
    // Remote sync: loadProviders self-gates on UserLlmProvidersRead.
    const reload = () => void actions.loadProviders()
    on('sync:user_llm_provider', reload)
    on('sync:reconnect', reload)
    void actions.loadProviders()
  },
})

export const useModelPickerStore = ModelPicker.store
