import { Stores } from '@/core/stores'

export const emitLlmProviderGroupsChanged = async (
  providerId: string,
  groupIds: string[],
) => {
  await Stores.EventBus.emit({
    type: 'llm_provider.groups_changed',
    data: { providerId, groupIds },
  })
}

export const emitGroupLlmProvidersChanged = async (
  groupId: string,
  providerIds: string[],
) => {
  await Stores.EventBus.emit({
    type: 'llm_provider.group_providers_changed',
    data: { groupId, providerIds },
  })
}
