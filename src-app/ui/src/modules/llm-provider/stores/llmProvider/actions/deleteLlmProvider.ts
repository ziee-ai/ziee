import { ApiClient } from '@/api-client'
import { emitLlmProviderDeleted } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (set: LlmProviderSet, get: LlmProviderGet) =>
  async (id: string) => {
    if (get().deleting) return
    try {
      set({ deleting: true, error: null })
      await ApiClient.LlmProvider.delete({ provider_id: id })
      try {
        await emitLlmProviderDeleted(id)
      } catch (eventError) {
        console.error('Failed to emit llm provider deleted event:', eventError)
      }
      set({ deleting: false })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to delete provider',
        deleting: false,
      })
      throw error
    }
  }
