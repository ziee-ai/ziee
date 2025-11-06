import { create } from 'zustand'
import type { LlmRepository } from '@/api-client/types'

interface LlmRepositoryDrawerState {
  open: boolean
  loading: boolean
  editingRepository: LlmRepository | null

  // Actions
  openDrawer: (repository?: LlmRepository) => void
  closeDrawer: () => void
  setLoading: (loading: boolean) => void
}

export const useLlmRepositoryDrawerStore = create<LlmRepositoryDrawerState>(
  (set) => ({
    open: false,
    loading: false,
    editingRepository: null,

    // Actions
    openDrawer: (repository?: LlmRepository) => {
      set({
        open: true,
        editingRepository: repository || null,
      })
    },

    closeDrawer: () => {
      set({
        open: false,
        loading: false,
        editingRepository: null,
      })
    },

    setLoading: (loading: boolean) => {
      set({ loading })
    },
  }),
)
