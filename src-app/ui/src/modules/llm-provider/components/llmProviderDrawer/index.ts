import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { llmProviderDrawerState, type LlmProviderDrawerState } from './state'
import type { Actions } from './actions.gen'

const LlmProviderDrawerDef = defineStore<LlmProviderDrawerState, Actions>('LlmProviderDrawer', {
  state: llmProviderDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, get, set, actions }) => {
    on('llm_provider.updated', event => {
      if (get().editingProvider?.id === event.data.provider.id) {
        set({ editingProvider: event.data.provider })
      }
    })
    on('llm_provider.deleted', event => {
      if (get().editingProvider?.id === event.data.providerId) actions.closeLlmProviderDrawer()
    })
  },
})
export const LlmProviderDrawer = registerLazyStore(LlmProviderDrawerDef)
export const useLlmProviderDrawerStore = LlmProviderDrawerDef.store
