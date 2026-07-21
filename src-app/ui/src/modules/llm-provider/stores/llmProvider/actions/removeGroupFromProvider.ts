import { ApiClient } from '@/api-client'
import { emitLlmProviderGroupsChanged } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (_set: LlmProviderSet, _get: LlmProviderGet) =>
  async (providerId: string, groupId: string) => {
    try {
      await ApiClient.LlmProvider.removeGroup({ provider_id: providerId, group_id: groupId })
      try {
        const groups = await ApiClient.LlmProvider.getGroups({ provider_id: providerId })
        await emitLlmProviderGroupsChanged(providerId, groups.map(g => g.id))
      } catch (eventError) {
        console.error('Failed to emit llm provider groups changed event:', eventError)
      }
    } catch (error) {
      console.error('Failed to remove group from provider:', error)
      throw error
    }
  }
