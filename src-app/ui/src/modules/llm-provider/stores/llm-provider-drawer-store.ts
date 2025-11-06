import { create } from 'zustand'
import type { LlmProvider } from '@/api-client/types'

interface LlmProviderDrawerState {
  isOpen: boolean
  editingProvider: LlmProvider | null

  // Actions
  openLlmProviderDrawer: (provider?: LlmProvider) => void
  closeLlmProviderDrawer: () => void
}

export const useLlmProviderDrawerStore = create<LlmProviderDrawerState>(
  (set): LlmProviderDrawerState => ({
    isOpen: false,
    editingProvider: null,

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
  }),
)
