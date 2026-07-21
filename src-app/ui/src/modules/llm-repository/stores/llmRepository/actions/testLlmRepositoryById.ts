import { ApiClient } from '@/api-client'
import type { UpdateLlmRepositoryRequest } from '@/api-client/types'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import emitLlmRepositoryUpdatedFactory from './_emitLlmRepositoryUpdated'

export default (set: LlmRepositorySet, get: LlmRepositoryGet) => {
  const emitUpdated = emitLlmRepositoryUpdatedFactory(set, get)
  return async (
    id: string,
    overrides: UpdateLlmRepositoryRequest,
  ): Promise<{ success: boolean; message: string }> => {
    if (get().testing) {
      return { success: false, message: 'Repository connection test already in progress' }
    }
    try {
      set({ testing: true, error: null })
      // Endpoint takes the row id + UpdateLlmRepositoryRequest body; changed
      // fields override, empty secrets fall back server-side.
      const result = await ApiClient.LlmRepository.testById({ repository_id: id, ...overrides })
      set({ testing: false })
      // The test persisted a fresh health status; re-fetch + emit `updated`
      // so the list + open drawer reflect it (SSE round-trip is unreliable).
      try {
        const fresh = await ApiClient.LlmRepository.get({ repository_id: id })
        await emitUpdated(fresh)
      } catch (refreshError) {
        console.error('Failed to refresh repository after connection test:', refreshError)
      }
      return result
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to test repository connection',
        testing: false,
      })
      throw error
    }
  }
}
