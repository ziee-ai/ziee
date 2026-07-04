import { ApiClient } from '@/api-client'
import type { HubModel } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const ModelDetailsDrawer = defineStore('ModelDetailsDrawer', {
  immer: true,
  state: {
    isOpen: false,
    selectedModel: null as HubModel | null,
    /** True while the fresh manifest is being fetched on open. */
    loading: false,
  },
  actions: (set, get) => ({
    open: (model: HubModel) => {
      set({ isOpen: true, selectedModel: model, loading: true })
      ApiClient.Hub.getManifest({ id: model.name, category: 'model' })
        .then(manifest => {
          if (get().isOpen && get().selectedModel?.name === model.name && manifest.model) {
            set({ selectedModel: manifest.model, loading: false })
          } else {
            set({ loading: false })
          }
        })
        .catch(() => set({ loading: false }))
    },
    close: () => set({ isOpen: false, selectedModel: null, loading: false }),
  }),
})

export const useModelDetailsDrawerStore = ModelDetailsDrawer.store
