import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { llmProviderState, type LlmProviderState } from './state'
import { sortProviders } from '@/modules/llm-provider/sortProviders'
import type { Actions } from './actions.gen'

const LlmProviderDef = defineStore<LlmProviderState, Actions>('LlmProvider', {
  immer: true,
  state: llmProviderState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, set, actions }) => {
    on('llm_provider.created', event => {
      const providerWithModels = { ...event.data.provider, llm_models: [] }
      set(state => ({ providers: sortProviders([...state.providers, providerWithModels]) }))
    })
    on('llm_provider.updated', event => {
      const { provider } = event.data
      set(state => {
        const existingProvider = state.providers.find(p => p.id === provider.id)
        const updatedProvider = {
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
export const LlmProvider = registerLazyStore(LlmProviderDef)
export const useLlmProviderStore = LlmProviderDef.store
