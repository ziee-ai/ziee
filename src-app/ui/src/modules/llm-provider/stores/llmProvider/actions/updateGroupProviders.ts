import { ApiClient } from '@/api-client'
import { emitGroupLlmProvidersChanged } from '@/modules/llm-provider/events'
import type { LlmProviderGet, LlmProviderSet } from '../state'

export default (_set: LlmProviderSet, _get: LlmProviderGet) =>
  async (groupId: string, providerIds: string[]) => {
    try {
      await ApiClient.Group.updateProviders({ group_id: groupId, provider_ids: providerIds })
      await emitGroupLlmProvidersChanged(groupId, providerIds)
    } catch (error) {
      console.error('Failed to update group providers:', error)
      throw error
    }
  }
