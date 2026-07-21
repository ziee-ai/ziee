import { ApiClient } from '@/api-client'
import type { VoiceRuntimeVersionGet, VoiceRuntimeVersionSet } from '../state'
import doLoadVersionsFactory from './_doLoadVersions'

export default (set: VoiceRuntimeVersionSet, get: VoiceRuntimeVersionGet) => {
  const doLoadVersions = doLoadVersionsFactory(set, get)
  return async () => {
    set({ loading: true, error: null })
    try {
      await ApiClient.Voice.syncVersionCache()
      await doLoadVersions()
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to sync cache',
        loading: false,
      })
    }
  }
}
