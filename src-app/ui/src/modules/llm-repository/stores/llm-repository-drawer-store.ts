import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { LlmRepository } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface LlmRepositoryDrawerState {
  open: boolean
  loading: boolean
  editingRepository: LlmRepository | null

  // Actions
  openDrawer: (repository?: LlmRepository) => void
  closeDrawer: () => void
  setLoading: (loading: boolean) => void

  // Initialization
  __init__: {
    __store__: () => void
  }
}

export const useLlmRepositoryDrawerStore = create<LlmRepositoryDrawerState>()(
  subscribeWithSelector(
    (set, get): LlmRepositoryDrawerState => ({
      open: false,
      loading: false,
      editingRepository: null,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus

          // Subscribe to llm_repository.updated
          eventBus.on('llm_repository.updated', async event => {
            const { repository } = event.data
            const state = get()

            if (state.editingRepository?.id === repository.id) {
              set({ editingRepository: repository })
            }
          })

          // Subscribe to llm_repository.deleted
          eventBus.on('llm_repository.deleted', async event => {
            const { repositoryId } = event.data
            const state = get()

            if (state.editingRepository?.id === repositoryId) {
              get().closeDrawer()
            }
          })
        },
      },

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
  ),
)
