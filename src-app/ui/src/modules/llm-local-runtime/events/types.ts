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

// A model's runtime usage changed (started/stopped/swapped to another
// version) — surfaces that show models-by-version or model run state should
// refresh.
export interface RuntimeModelUsageChangedEvent extends BaseEvent {
  data: {
    modelId: string
  }
}

declare module '@/core/events' {
  interface AppEvents {
    'runtime_version.created': RuntimeVersionCreatedEvent
    'runtime_version.deleted': RuntimeVersionDeletedEvent
    'runtime_version.default_changed': RuntimeVersionDefaultChangedEvent
    'runtime_version.usage_changed': RuntimeModelUsageChangedEvent
  }
}
