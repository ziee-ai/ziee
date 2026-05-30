import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubLocalProvider, HubModel } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface HubModelsState {
  models: HubModel[]
  version: string | null
  loading: boolean
  error: string | null

  localProviders: HubLocalProvider[]
  localProvidersLoaded: boolean

  // Actions
  loadModels: (force?: boolean) => Promise<void>
  refreshFromGitHub: () => Promise<void>
  loadLocalProviders: () => Promise<void>
  downloadModelFromHub: (
    hubId: string,
    providerId: string,
    displayName: string,
    quantizationName?: string,
  ) => Promise<void>

  // Lazy initialization
  __init__: {
    models: () => Promise<void>
    localProviders: () => Promise<void>
    __store__?: () => void
  }
  __destroy__?: () => void
}

export const useHubModelsStore = create<HubModelsState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubModelsState => ({
        models: [],
        version: null,
        loading: false,
        error: null,
        localProviders: [],
        localProvidersLoaded: false,

        loadModels: async (force = false) => {
          const state = get()
          if (state.loading && !force) return

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

        loadLocalProviders: async () => {
          const state = get()
          if (state.localProvidersLoaded) return
          try {
            const response = await ApiClient.Hub.getLocalProviders()
            set({ localProviders: response.providers, localProvidersLoaded: true })
          } catch {
            set({ localProvidersLoaded: true })
          }
        },

        downloadModelFromHub: async (
          hubId: string,
          providerId: string,
          displayName: string,
          quantizationName?: string,
        ) => {
          const result = await ApiClient.Hub.createModelFromHub({
            hub_id: hubId,
            provider_id: providerId,
            display_name: displayName,
            quantization_name: quantizationName,
          })
          Stores.LlmModelDownload.addExternalDownload(result.download)
        },

        __init__: {
          __store__: () => {
            Stores.EventBus.on(
              'llm_model.deleted',
              event => {
                const { modelId } = event.data
                set(state => {
                  for (const model of state.models) {
                    if (model.created_ids) {
                      model.created_ids = model.created_ids.filter(
                        id => id !== modelId,
                      )
                    }
                  }
                })
              },
              'HubModelsStore',
            )
          },
          models: () => get().loadModels(),
          localProviders: () => get().loadLocalProviders(),
        },

        // Unsubscribe from EventBus on store destroy so listener slots
        // don't accumulate per destroy/re-init cycle. Mirrors the
        // pattern in ChatHistory.store.ts. (audit 09 B-9)
        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('HubModelsStore')
        },
      }),
    ),
  ),
)
