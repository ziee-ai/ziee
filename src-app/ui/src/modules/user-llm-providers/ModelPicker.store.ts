import { ApiClient } from '@/api-client'
import { Permissions, type ProviderWithModels } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import { sortProviders } from '@/modules/llm-provider/sortProviders'

/**
 * The composer key for a not-yet-created (new-chat) conversation. A pane with no
 * conversation yet selects its model under this key; on send the created
 * conversation adopts it (see the model chat-extension's composeRequestFields).
 * Two simultaneous new-chat panes share this one slot (accepted edge — the
 * per-pane split value is comparing EXISTING conversations' models).
 */
export const NEW_CHAT_MODEL_KEY = '__new_chat__'

/**
 * ModelPicker store — the chat composer's model selection plus the cached list
 * of user-accessible providers/models. Lives at `Stores.ModelPicker`. `providers`
 * is a GLOBAL catalog (lazy-loads once; listeners keep it in sync with admin
 * llm_provider/llm_model mutations). The SELECTION is PER-CONVERSATION
 * (`selectedByConversation`, keyed by conversation id or `NEW_CHAT_MODEL_KEY`) so
 * two split panes showing different conversations each pick their own model
 * (compare-models side-by-side) — ITEM-5. Consumers key by their pane's
 * conversation id (resolved via the `Stores.Chat` bridge); the send path threads
 * the pane's chat `get` into `composeRequestFields`.
 */
export const ModelPicker = defineStore('ModelPicker', {
  immer: true,
  state: {
    /** User-accessible providers from the chat endpoint (GLOBAL catalog). */
    providers: [] as ProviderWithModels[],
    loading: false,
    error: null as string | null,
    /** Selected model ID (UUID) per conversation key (convId | NEW_CHAT_MODEL_KEY). */
    selectedByConversation: {} as Record<string, string>,
  },
  actions: (set, get) => {
    const firstEnabledModelId = (): string | null => {
      for (const provider of get().providers) {
        const firstEnabled = provider.llm_models?.find(m => m.enabled)
        if (firstEnabled) return firstEnabled.id
      }
      return null
    }
    const initializeFromConversation = (
      key: string,
      conversationModelId?: string,
    ) => {
      const providers = get().providers
      // Prefer the conversation's own (enabled) model; else the first enabled.
      let resolved: string | null = null
      if (conversationModelId) {
        for (const provider of providers) {
          const match = provider.llm_models?.find(
            m => m.id === conversationModelId && m.enabled,
          )
          if (match) {
            resolved = match.id
            break
          }
        }
      }
      resolved = resolved ?? firstEnabledModelId()
      if (resolved) {
        set(state => {
          state.selectedByConversation[key] = resolved as string
        })
      }
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
        // Seed the new-chat default if unset yet.
        if (!get().selectedByConversation[NEW_CHAT_MODEL_KEY]) {
          initializeFromConversation(NEW_CHAT_MODEL_KEY)
        }
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
      /** Set the selected model for one conversation key. */
      setModelId: (key: string, id: string) => {
        set(state => {
          state.selectedByConversation[key] = id
        })
      },
      /** Get the selected model for a conversation key (null if unset). */
      getModelId: (key: string): string | null =>
        get().selectedByConversation[key] ?? null,
      /** The new-chat default model — for non-pane consumers (e.g. the workflow
       *  run dialog) that just need a sensible current model. */
      defaultModelId: (): string | null =>
        get().selectedByConversation[NEW_CHAT_MODEL_KEY] ?? firstEnabledModelId(),
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
    // Prune a deleted conversation's per-conversation model selection so the
    // `selectedByConversation` map doesn't grow unbounded / retain stale keys.
    on('sync:conversation', event => {
      if (event.data.action === 'delete') {
        set(state => {
          delete state.selectedByConversation[event.data.id]
        })
      }
    })
    void actions.loadProviders()
  },
})

export const useModelPickerStore = ModelPicker.store
