import { ApiClient } from '@/api-client'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import emitLlmRepositoryDeletedFactory from './_emitLlmRepositoryDeleted'

export default (set: LlmRepositorySet, get: LlmRepositoryGet) => {
  const emitDeleted = emitLlmRepositoryDeletedFactory(set, get)
  return async (id: string) => {
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.LlmRepository.delete({ repository_id: id })
      try {
        await emitDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit llm repository deleted event:', eventError)
      }
      set({ deleting: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete repository',
        deleting: false,
      })
      throw error
    }
  }
}
