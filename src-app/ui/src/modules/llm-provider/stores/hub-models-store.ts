import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubModel } from '@/api-client/types'

interface HubModelsState {
  models: HubModel[]
  version: string | null
  loading: boolean
  error: string | null

  // Actions
  loadModels: () => Promise<void>
  refreshFromGitHub: () => Promise<void>

  // Lazy initialization
  __init__: {
    models: () => Promise<void>
  }
}

export const useHubModelsStore = create<HubModelsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubModelsState => ({
        models: [],
        version: null,
        loading: false,
        error: null,

        loadModels: async () => {
          const state = get()
          if (state.loading) return

          set({ loading: true, error: null })
          try {
            // Load with user's locale
            const locale = 'en' // TODO: Get from user settings
            const models = await ApiClient.Hub.getModels({ lang: locale })
            const versionInfo = await ApiClient.Hub.getModelsVersion()

            set({
              models,
              version: versionInfo.version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to load hub models',
              loading: false,
            })
          }
        },

        refreshFromGitHub: async () => {
          set({ loading: true, error: null })
          try {
            // Call category-specific refresh endpoint
            const result = await ApiClient.Hub.refreshModels()

            // Reload if updated
            if (result.updated) {
              await get().loadModels()
            }

            set({ loading: false })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to refresh hub models',
              loading: false,
            })
            throw error
          }
        },

        __init__: {
          models: () => get().loadModels(),
        },
      })
    )
  )
)
