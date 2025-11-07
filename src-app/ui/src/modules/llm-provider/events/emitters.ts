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
