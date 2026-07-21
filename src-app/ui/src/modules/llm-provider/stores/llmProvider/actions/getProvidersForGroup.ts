import { ApiClient } from '@/api-client'
import type { LlmProviderGet, LlmProviderSet } from '../state'
import type { LlmProvider } from '@/api-client/types'

export default (_set: LlmProviderSet, _get: LlmProviderGet) =>
  async (groupId: string): Promise<LlmProvider[]> => {
    try {
      const response = await ApiClient.Group.getProviders({ group_id: groupId })
      // Guard: callers `.map` the result — never hand back undefined.
      return Array.isArray(response.providers) ? response.providers : []
    } catch (error) {
      console.error('Failed to get providers for group:', error)
      throw error
    }
  }
