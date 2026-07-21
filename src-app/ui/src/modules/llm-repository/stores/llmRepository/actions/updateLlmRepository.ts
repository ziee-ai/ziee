import { ApiClient } from '@/api-client'
import type { LlmRepository, UpdateLlmRepositoryRequest } from '@/api-client/types'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import emitLlmRepositoryUpdatedFactory from './_emitLlmRepositoryUpdated'
import emitLlmRepositoryAutoDisabledFactory from './_emitLlmRepositoryAutoDisabled'

export default (set: LlmRepositorySet, get: LlmRepositoryGet) => {
  const emitUpdated = emitLlmRepositoryUpdatedFactory(set, get)
  const emitAutoDisabled = emitLlmRepositoryAutoDisabledFactory(set, get)
  return async (id: string, data: UpdateLlmRepositoryRequest): Promise<LlmRepository> => {
    if (get().updating) return Promise.resolve(null as any)
    try {
      set({ updating: true, error: null })
      const repository = await ApiClient.LlmRepository.update({ repository_id: id, ...data })
      try {
        await emitUpdated(repository)
      } catch (eventError) {
        console.error('Failed to emit llm repository updated event:', eventError)
      }
      set({ updating: false })
      return repository
    } catch (error) {
      // An enable-transition probe failure (400) leaves the row disabled +
      // `unhealthy` server-side. Emit auto_disabled so the list reloads
      // deterministically without waiting on the SSE round-trip.
      const code = (error as { error_code?: string })?.error_code
      if (code === 'LLM_REPOSITORY_ENABLE_FAILED_HEALTH_CHECK') {
        try {
          await emitAutoDisabled(
            id,
            error instanceof Error ? error.message : 'Connection probe failed',
          )
        } catch (eventError) {
          console.error('Failed to emit llm repository auto_disabled event:', eventError)
        }
      }
      set({
        error: error instanceof Error ? error.message : 'Failed to update repository',
        updating: false,
      })
      throw error
    }
  }
}
