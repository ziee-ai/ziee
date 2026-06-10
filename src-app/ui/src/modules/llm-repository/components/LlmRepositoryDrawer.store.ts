import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
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
  __destroy__?: () => void
}

export const useLlmRepositoryDrawerStore = create<LlmRepositoryDrawerState>()(
  subscribeWithSelector(
    (set, get): LlmRepositoryDrawerState => ({
      open: false,
      loading: false,
      editingRepository: null,

      __init__: {
        __store__: () => {
          const GROUP = 'LlmRepositoryDrawerStore'
          const eventBus = Stores.EventBus

          // Subscribe to llm_repository.updated
          eventBus.on(
            'llm_repository.updated',
            async event => {
              const { repository } = event.data
              const state = get()

              if (state.editingRepository?.id === repository.id) {
                set({ editingRepository: repository })
              }
            },
            GROUP,
          )

          // Subscribe to llm_repository.deleted
          eventBus.on(
            'llm_repository.deleted',
            async event => {
              const { repositoryId } = event.data
              const state = get()

              if (state.editingRepository?.id === repositoryId) {
                get().closeDrawer()
              }
            },
            GROUP,
          )

          // Subscribe to llm_repository.auto_disabled — an enable probe
          // failed, so the row was disabled + marked `unhealthy`
          // server-side. Re-fetch the canonical row so the open edit
          // drawer reflects enabled=false / unhealthy and renders the
          // inline Alert (the list reload alone doesn't touch the drawer's
          // own `editingRepository` snapshot).
          eventBus.on(
            'llm_repository.auto_disabled',
            async event => {
              const { repositoryId } = event.data
              if (get().editingRepository?.id !== repositoryId) {
                return
              }
              try {
                const fresh = await ApiClient.LlmRepository.get({
                  repository_id: repositoryId,
                })
                // Guard against a close / switch during the await.
                if (get().editingRepository?.id === repositoryId) {
                  set({ editingRepository: fresh })
                }
              } catch (err) {
                console.error(
                  'Failed to refresh auto-disabled repository in drawer:',
                  err,
                )
              }
            },
            GROUP,
          )
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

      __destroy__: () => {
        Stores.EventBus.removeGroupListeners('LlmRepositoryDrawerStore')
      },
    }),
  ),
)
