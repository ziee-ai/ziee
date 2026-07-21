import { ApiClient } from '@/api-client'
import type {
  CreateLlmRepositoryRequest,
  LlmRepositoryWithHealthWarning,
} from '@/api-client/types'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import emitLlmRepositoryCreatedFactory from './_emitLlmRepositoryCreated'
import emitLlmRepositoryAutoDisabledFactory from './_emitLlmRepositoryAutoDisabled'

export default (set: LlmRepositorySet, get: LlmRepositoryGet) => {
  const emitCreated = emitLlmRepositoryCreatedFactory(set, get)
  const emitAutoDisabled = emitLlmRepositoryAutoDisabledFactory(set, get)
  return async (data: CreateLlmRepositoryRequest): Promise<LlmRepositoryWithHealthWarning> => {
    if (get().creating) return Promise.resolve(null as any)
    try {
      set({ creating: true, error: null })
      // Response `{ repository, connection_warning? }` (flattened): the
      // backend probes when enabled:true and auto-flips on failure.
      const wrapped = await ApiClient.LlmRepository.create(data)
      try {
        await emitCreated(wrapped)
      } catch (eventError) {
        console.error('Failed to emit llm repository created event:', eventError)
      }
      // On downgrade, also emit auto_disabled so the settings page reloads
      // and renders the `unhealthy` Alert.
      if (wrapped.connection_warning) {
        try {
          await emitAutoDisabled(wrapped.id, wrapped.connection_warning.reason)
        } catch (eventError) {
          console.error('Failed to emit llm repository auto_disabled event:', eventError)
        }
      }
      set({ creating: false })
      return wrapped
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to create repository',
        creating: false,
      })
      throw error
    }
  }
}
