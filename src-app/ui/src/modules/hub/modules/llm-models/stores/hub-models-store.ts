import { ApiClient } from '@/api-client'
import type { HubLocalProvider, HubModel } from '@/api-client/types'
import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { LlmModelDownload } from '@/modules/llm-provider/stores/llmModelDownload'

const HubModelsDef = defineStore('HubModels', {
  immer: true,
  state: {
    models: [] as HubModel[],
    version: null as string | null,
    loading: false,
    error: null as string | null,
    localProviders: [] as HubLocalProvider[],
    localProvidersLoaded: false,
  },
  actions: (set, get) => {
    const loadModels = async (force = false) => {
      if (get().loading && !force) return
      set({ loading: true, error: null })
      try {
        const locale = 'en' // TODO: Get from user settings
        const models = await ApiClient.Hub.getModels({ lang: locale })
        const versionInfo = await ApiClient.Hub.getModelsVersion()
        set({ models, version: versionInfo.version, loading: false })
      } catch (error: any) {
        set({ error: error.message || 'Failed to load hub models', loading: false })
      }
    }
    const loadLocalProviders = async () => {
      if (get().localProvidersLoaded) return
      try {
        const response = await ApiClient.Hub.getLocalProviders()
        set({ localProviders: response.providers, localProvidersLoaded: true })
      } catch {
        set({ localProvidersLoaded: true })
      }
    }
    return {
      loadModels,
      loadLocalProviders,
      refreshFromGitHub: async () => {
        set({ loading: true, error: null })
        try {
          const result = await ApiClient.Hub.refreshModels()
          if (result.updated) await loadModels()
          set({ loading: false })
        } catch (error: any) {
          set({ error: error.message || 'Failed to refresh hub models', loading: false })
          throw error
        }
      },
      downloadModelFromHub: async (
        hubId: string,
        providerId: string,
        displayName: string,
        quantizationName?: string,
        sourceIndex?: number,
      ) => {
        const result = await ApiClient.Hub.createModelFromHub({
          hub_id: hubId,
          provider_id: providerId,
          display_name: displayName,
          quantization_name: quantizationName,
          // v2 Phase 7: source_index picks which sources[] entry to install
          // from. Defaults to 0 server-side when omitted.
          source_index: sourceIndex,
        })
        LlmModelDownload.addExternalDownload(result.download)
      },
    }
  },
  init: ({ on, set, actions }) => {
    on('llm_model.deleted', event => {
      const { modelId } = event.data
      set(state => {
        for (const model of state.models) {
          if (model.created_ids) {
            model.created_ids = model.created_ids.filter(id => id !== modelId)
          }
        }
      })
    })
    void actions.loadModels()
    void actions.loadLocalProviders()
  },
})

export const useHubModelsStore = HubModelsDef.store

export const HubModels = registerLazyStore(HubModelsDef)
