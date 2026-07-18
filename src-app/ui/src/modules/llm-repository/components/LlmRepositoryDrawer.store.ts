import { ApiClient } from '@/api-client'
import type { LlmRepository } from '@/api-client/types'
import { defineStore } from '@ziee/framework/store-kit'

export const LlmRepositoryDrawer = defineStore('LlmRepositoryDrawer', {
  state: {
    open: false,
    loading: false,
    editingRepository: null as LlmRepository | null,
  },
  actions: set => ({
    openDrawer: (repository?: LlmRepository) =>
      set({ open: true, editingRepository: repository || null }),
    closeDrawer: () => set({ open: false, loading: false, editingRepository: null }),
    setLoading: (loading: boolean) => set({ loading }),
  }),
  init: ({ on, get, set, actions }) => {
    on('llm_repository.updated', event => {
      if (get().editingRepository?.id === event.data.repository.id) {
        set({ editingRepository: event.data.repository })
      }
    })
    on('llm_repository.deleted', event => {
      if (get().editingRepository?.id === event.data.repositoryId) actions.closeDrawer()
    })
    // auto_disabled: an enable probe failed → row disabled + marked unhealthy
    // server-side. Re-fetch the canonical row so the open edit drawer reflects
    // enabled=false / unhealthy and renders the inline Alert.
    on('llm_repository.auto_disabled', async event => {
      const { repositoryId } = event.data
      if (get().editingRepository?.id !== repositoryId) return
      try {
        const fresh = await ApiClient.LlmRepository.get({ repository_id: repositoryId })
        if (get().editingRepository?.id === repositoryId) set({ editingRepository: fresh })
      } catch (err) {
        console.error('Failed to refresh auto-disabled repository in drawer:', err)
      }
    })
  },
})

export const useLlmRepositoryDrawerStore = LlmRepositoryDrawer.store
