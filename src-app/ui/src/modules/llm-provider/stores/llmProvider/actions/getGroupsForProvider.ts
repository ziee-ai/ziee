import { ApiClient } from '@/api-client'
import type { Group } from '@/api-client/types'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (_set: LlmProviderSet, _get: LlmProviderGet) =>
  async (providerId: string): Promise<Group[]> => {
    try {
      return await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
    } catch (error) {
      console.error('Failed to get groups for provider:', error)
      throw error
    }
  }
