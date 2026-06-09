import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { Permissions, type ProviderWithModels } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'
import { sortProviders } from '@/modules/llm-provider/sortProviders'

/**
 * ModelPicker store — the chat composer's model selection plus the
 * cached list of user-accessible providers/models used by the picker.
 * Lives at Stores.ModelPicker (registered in
 * modules/user-llm-providers/module.tsx). Prior name was
 * Stores.Chat.ModelStore (nested via the chat-extension framework);
 * relocated out so the model-domain state lives in the
 * user-llm-providers module that owns it.
 *
 * Lifecycle:
 *   - providers lazy-loads once via __init__ on first access (the
 *     chat composer fetches it when it mounts).
 *   - EventBus subscribers in __init__.__store__ keep the cached
 *     providers list in sync with admin-side llm_provider/llm_model
 *     mutations; cleanup runs in __destroy__.
 *   - selectedModelId is scoped to the active chat conversation; the
 *     chat-extension's initialize() hook calls
 *     initializeFromConversation(conversation.model_id) on
 *     conversation-id change (replaces the implicit
 *     createExtensionStore auto-scoping).
 */
interface ModelPickerState {
  // Provider data (merged from ChatLlmProvider)
  /** User-accessible providers from chat endpoint */
  providers: ProviderWithModels[]

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

export const useModelPickerStore = create<ModelPickerState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      // Provider state (merged from ChatLlmProvider)
      providers: [],
      loading: false,
      error: null,

      // Model selection state
      selectedModelId: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus
          const GROUP = 'ModelPicker'

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
            },
            GROUP,
          )

          // Subscribe to llm_provider.deleted
          eventBus.on(
            'llm_provider.deleted',
            async event => {
              const { providerId } = event.data
              set(state => {
                state.providers = state.providers.filter(
                  p => p.id !== providerId,
                )
              })
            },
            GROUP,
          )

          // Subscribe to llm_model.enabled
          eventBus.on(
            'llm_model.enabled',
            async event => {
              const { modelId } = event.data
              set(state => {
                state.providers = state.providers.map(p => ({
                  ...p,
                  llm_models: p.llm_models?.map(m =>
                    m.id === modelId ? { ...m, enabled: true } : m,
                  ),
                }))
              })
            },
            GROUP,
          )

          // Subscribe to llm_model.disabled
          eventBus.on(
            'llm_model.disabled',
            async event => {
              const { modelId } = event.data
              set(state => {
                state.providers = state.providers.map(p => ({
                  ...p,
                  llm_models: p.llm_models?.map(m =>
                    m.id === modelId ? { ...m, enabled: false } : m,
                  ),
                }))
              })
            },
            GROUP,
          )

          // Subscribe to llm_model.deleted
          eventBus.on(
            'llm_model.deleted',
            async event => {
              const { modelId } = event.data
              set(state => {
                state.providers = state.providers.map(p => ({
                  ...p,
                  llm_models: p.llm_models?.filter(m => m.id !== modelId),
                }))
              })
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

          // Remote sync: an admin changed a provider/model on another
          // device, or we (re)connected. `loadProviders()` self-gates on
          // UserLlmProvidersRead and refetches its own scoped view.
          const reload = () => void get().loadProviders()
          eventBus.on('sync:user_llm_provider', reload, GROUP)
          eventBus.on('sync:reconnect', reload, GROUP)
        },
        providers: () => get().loadProviders(),
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('ModelPicker')
      },

      // Load user-accessible providers (merged from ChatLlmProvider)
      loadProviders: async () => {
        // Permission-gate the shell-eager-load fetch (audit
        // follow-up): the chat model picker accesses this on every
        // chat render. The endpoint is gated on user_llm_providers::
        // read; without it the call 403s.
        if (!hasPermissionNow(Permissions.UserLlmProvidersRead)) return

        set(state => {
          state.loading = true
          state.error = null
        })
        try {
          const response = await ApiClient.LlmProvider.getUserLlmProviders(
            undefined,
            undefined,
          )
          set(state => {
            state.providers = sortProviders(response.providers)
            state.loading = false
          })
          // Auto-select first model if none is selected yet
          if (!get().selectedModelId) {
            get().initializeFromConversation()
          }
        } catch (error: any) {
          console.error('[ModelPicker] loadProviders error:', error)
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
    })),
  ),
)
