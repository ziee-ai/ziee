import { ApiClient } from '@/api-client'
import type { VoiceRuntimeVersionGet, VoiceRuntimeVersionSet } from '../state'

export default (set: VoiceRuntimeVersionSet, _get: VoiceRuntimeVersionGet) =>
  async () => {
    set({ loading: true, error: null })
    try {
      const response = await ApiClient.Voice.listVersions({})
      set({
        versions: response.versions || [],
        isInitialized: true,
        loading: false,
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load versions',
        loading: false,
      })
    }
  }
