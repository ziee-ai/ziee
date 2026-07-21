import { ApiClient } from '@/api-client'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, _get: LlmProviderGet) =>
  async (providerId: string) => {
    set(state => ({
      discoverLoading: { ...state.discoverLoading, [providerId]: true },
    }))
    try {
      const resp = await ApiClient.LlmProvider.discoverModels({ provider_id: providerId })
      set(state => ({
        discoveredModels: { ...state.discoveredModels, [providerId]: resp.models },
        discoverNotes: { ...state.discoverNotes, [providerId]: resp.notes },
        discoverLoading: { ...state.discoverLoading, [providerId]: false },
      }))
      return resp.models
    } catch (error) {
      set(state => ({
        discoveredModels: { ...state.discoveredModels, [providerId]: [] },
        discoverNotes: {
          ...state.discoverNotes,
          [providerId]: [
            error instanceof Error ? error.message : 'Failed to discover models',
          ],
        },
        discoverLoading: { ...state.discoverLoading, [providerId]: false },
      }))
      return []
    }
  }
