import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { modelPickerState, type ModelPickerState } from './state'
import type { Actions } from './actions.gen'

const ModelPickerDef = defineStore<ModelPickerState, Actions>('ModelPicker', {
  immer: true,
  state: modelPickerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, set, actions }) => {
    on('llm_provider.created', () => void actions.loadProviders())
    on('llm_provider.updated', event => {
      const { provider } = event.data
      set(state => {
        const existingProvider = state.providers.find(p => p.id === provider.id)
        if (!existingProvider) return
        const updatedProvider: import('@/api-client/types').ProviderWithModels = {
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

export const ModelPicker = registerLazyStore(ModelPickerDef)
export const useModelPickerStore = ModelPickerDef.store

// Re-export constants so consumers don't need to import from state.ts directly.
export { NEW_CHAT_MODEL_KEY, newChatModelKey } from './state'
