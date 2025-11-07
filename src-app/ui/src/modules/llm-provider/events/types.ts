import type { BaseEvent } from '@/core/events'

export interface LlmProviderGroupsChangedEvent extends BaseEvent {
  type: 'llm_provider.groups_changed'
  data: {
    providerId: string
    groupIds: string[]
  }
}

export interface GroupLlmProvidersChangedEvent extends BaseEvent {
  type: 'llm_provider.group_providers_changed'
  data: {
    groupId: string
    providerIds: string[]
  }
}

export type LlmProviderModuleEvent =
  | LlmProviderGroupsChangedEvent
  | GroupLlmProvidersChangedEvent

declare module '@/core/events' {
  interface AppEvents {
    'llm_provider.groups_changed': LlmProviderGroupsChangedEvent
    'llm_provider.group_providers_changed': GroupLlmProvidersChangedEvent
  }
}
