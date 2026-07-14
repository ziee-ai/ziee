import type { BaseEvent } from '@ziee/framework/events'
import type {
  MemoryAdminSettings,
  UserMemory,
  UserMemorySettings,
} from '@/api-client/types'

export interface MemoryCreatedEvent extends BaseEvent {
  type: 'memory.created'
  data: { memory: UserMemory }
}
export interface MemoryUpdatedEvent extends BaseEvent {
  type: 'memory.updated'
  data: { memory: UserMemory }
}
export interface MemoryDeletedEvent extends BaseEvent {
  type: 'memory.deleted'
  data: { memoryId: string }
}
export interface MemoryAllClearedEvent extends BaseEvent {
  type: 'memory.all_cleared'
  data: { deletedCount: number }
}
export interface MemorySettingsUpdatedEvent extends BaseEvent {
  type: 'memory.settings_updated'
  data: { settings: UserMemorySettings }
}
export interface MemoryAdminSettingsUpdatedEvent extends BaseEvent {
  type: 'memory.admin_settings_updated'
  data: { settings: MemoryAdminSettings }
}

export type MemoryModuleEvent =
  | MemoryCreatedEvent
  | MemoryUpdatedEvent
  | MemoryDeletedEvent
  | MemoryAllClearedEvent
  | MemorySettingsUpdatedEvent
  | MemoryAdminSettingsUpdatedEvent

declare module '@ziee/framework/events' {
  interface AppEvents {
    'memory.created': MemoryCreatedEvent
    'memory.updated': MemoryUpdatedEvent
    'memory.deleted': MemoryDeletedEvent
    'memory.all_cleared': MemoryAllClearedEvent
    'memory.settings_updated': MemorySettingsUpdatedEvent
    'memory.admin_settings_updated': MemoryAdminSettingsUpdatedEvent
  }
}
