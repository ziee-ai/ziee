import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubAssistant } from '@/api-client/types'

interface HubAssistantsState {
  assistants: HubAssistant[]
  version: string | null
  loading: boolean
  error: string | null

  // Actions
  loadAssistants: () => Promise<void>
  refreshFromGitHub: () => Promise<void>

  // Lazy initialization
  __init__: {
    assistants: () => Promise<void>
  }
}

export const useHubAssistantsStore = create<HubAssistantsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubAssistantsState => ({
        assistants: [],
        version: null,
        loading: false,
        error: null,

        loadAssistants: async () => {
          const state = get()
          if (state.loading) return

          set({ loading: true, error: null })
          try {
            // Load with user's locale
            const locale = 'en' // TODO: Get from user settings
            const assistants = await ApiClient.Hub.getAssistants({ lang: locale })
            const versionInfo = await ApiClient.Hub.getAssistantsVersion()

            set({
              assistants,
              version: versionInfo.version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to load hub assistants',
              loading: false,
            })
          }
        },

        refreshFromGitHub: async () => {
          set({ loading: true, error: null })
          try {
            // Call category-specific refresh endpoint
            const result = await ApiClient.Hub.refreshAssistants()

            // Reload if updated
            if (result.updated) {
              await get().loadAssistants()
            }

            set({ loading: false })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to refresh hub assistants',
              loading: false,
            })
            throw error
          }
        },

        __init__: {
          assistants: () => get().loadAssistants(),
        },
      })
    )
  )
)
