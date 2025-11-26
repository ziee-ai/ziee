import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { LlmProvider } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface LlmProviderDrawerState {
  isOpen: boolean
  editingProvider: LlmProvider | null

  // Actions
  openLlmProviderDrawer: (provider?: LlmProvider) => void
  closeLlmProviderDrawer: () => void

  // Initialization
  __init__: {
    __store__: () => void
  }
  __destroy__?: () => void
}

export const useLlmProviderDrawerStore = create<LlmProviderDrawerState>()(
  subscribeWithSelector(
    (set, get): LlmProviderDrawerState => ({
      isOpen: false,
      editingProvider: null,

      __init__: {
        __store__: () => {
          const GROUP = 'LlmProviderDrawerStore'
          const eventBus = Stores.EventBus

          // Subscribe to llm_provider.updated
          eventBus.on(
            'llm_provider.updated',
            async event => {
              const { provider } = event.data
              const state = get()

              if (state.editingProvider?.id === provider.id) {
                set({ editingProvider: provider })
              }
            },
            GROUP,
          )

          // Subscribe to llm_provider.deleted
          eventBus.on(
            'llm_provider.deleted',
            async event => {
              const { providerId } = event.data
              const state = get()

              if (state.editingProvider?.id === providerId) {
                get().closeLlmProviderDrawer()
              }
            },
            GROUP,
          )
        },
      },

      // Actions
      openLlmProviderDrawer: (provider?: LlmProvider) => {
        set({
          isOpen: true,
          editingProvider: provider ?? null,
        })
      },

      closeLlmProviderDrawer: () => {
        set({ isOpen: false, editingProvider: null })
      },

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('LlmProviderDrawerStore')
      },
    }),
  ),
)
