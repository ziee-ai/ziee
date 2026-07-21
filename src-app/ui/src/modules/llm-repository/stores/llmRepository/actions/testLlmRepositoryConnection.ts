import { ApiClient } from '@/api-client'
import type { TestRepositoryConnectionRequest } from '@/api-client/types'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'

export default (set: LlmRepositorySet, get: LlmRepositoryGet) =>
  async (data: TestRepositoryConnectionRequest): Promise<{ success: boolean; message: string }> => {
    if (get().testing) {
      return { success: false, message: 'Repository connection test already in progress' }
    }
    try {
      set({ testing: true, error: null })
      const result = await ApiClient.LlmRepository.test(data)
      set({ testing: false })
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
