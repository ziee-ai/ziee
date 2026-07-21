import { ApiClient } from '@/api-client'
import type { HubModel } from '@/api-client/types'
import type { ModelDetailsDrawerGet, ModelDetailsDrawerSet } from '../state'

export default (set: ModelDetailsDrawerSet, get: ModelDetailsDrawerGet) =>
  async (model: HubModel) => {
    set({ isOpen: true, selectedModel: model, loading: true })
    try {
      const manifest = await ApiClient.Hub.getManifest({
        id: model.name,
        category: 'model',
      })
      if (get().isOpen && get().selectedModel?.name === model.name && manifest.model) {
        set({ selectedModel: manifest.model, loading: false })
      } else {
        set({ loading: false })
      }
    } catch {
      set({ loading: false })
    }
  }
