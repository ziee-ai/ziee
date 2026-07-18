import type { LlmProvider } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const LlmProviderDrawer = defineStore('LlmProviderDrawer', {
  state: { isOpen: false, editingProvider: null as LlmProvider | null },
  actions: set => ({
    openLlmProviderDrawer: (provider?: LlmProvider) =>
      set({ isOpen: true, editingProvider: provider ?? null }),
    closeLlmProviderDrawer: () => set({ isOpen: false, editingProvider: null }),
  }),
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

export const useLlmProviderDrawerStore = LlmProviderDrawer.store
