import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubModel } from '@/api-client/types'

interface ModelDetailsDrawerState {
  isOpen: boolean
  selectedModel: HubModel | null
  /** True while the fresh manifest is being fetched on open. */
  loading: boolean

  // Actions
  open: (model: HubModel) => void
  close: () => void
}

export const useModelDetailsDrawerStore = create<ModelDetailsDrawerState>()(
  immer(
    (set, get): ModelDetailsDrawerState => ({
      isOpen: false,
      selectedModel: null,
      loading: false,

      // Render the list's copy immediately for snappiness, then fetch
      // the authoritative manifest from current/ via /api/hub/manifest
      // so the drawer always reflects the active catalog version.
      open: (model: HubModel) => {
        set({ isOpen: true, selectedModel: model, loading: true })
        ApiClient.Hub.getManifest({ id: model.name, category: 'model' })
          .then(manifest => {
            // Ignore if the user already closed or switched items.
            if (
              get().isOpen &&
              get().selectedModel?.name === model.name &&
              manifest.model
            ) {
              set({ selectedModel: manifest.model, loading: false })
            } else {
              set({ loading: false })
            }
          })
          .catch(() => set({ loading: false }))
      },

      close: () => {
        set({ isOpen: false, selectedModel: null, loading: false })
      },
    }),
  ),
)
