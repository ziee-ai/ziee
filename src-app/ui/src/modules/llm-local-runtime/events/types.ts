import type { BaseEvent } from '@/core/events'
import type { RuntimeVersionResponse } from '@/api-client/types'

export interface RuntimeVersionCreatedEvent extends BaseEvent {
  data: {
    version: RuntimeVersionResponse
  }
}

export interface RuntimeVersionDeletedEvent extends BaseEvent {
  data: {
    versionId: string
  }
}

export interface RuntimeVersionDefaultChangedEvent extends BaseEvent {
  data: {
    versionId: string
  }
}

declare module '@/core/events' {
  interface AppEvents {
    'runtime_version.created': RuntimeVersionCreatedEvent
    'runtime_version.deleted': RuntimeVersionDeletedEvent
    'runtime_version.default_changed': RuntimeVersionDefaultChangedEvent
  }
}
