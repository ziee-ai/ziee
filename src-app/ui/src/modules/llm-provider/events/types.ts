import type { BaseEvent } from '@/core/events'

export interface LlmProviderGroupsChangedEvent extends BaseEvent {
  type: 'llm_provider.groups_changed'
  data: {
    providerId: string
    groupIds: string[]
  }
}

export type LlmProviderModuleEvent = LlmProviderGroupsChangedEvent

declare module '@/core/events' {
  interface AppEvents {
    'llm_provider.groups_changed': LlmProviderGroupsChangedEvent
  }
}
