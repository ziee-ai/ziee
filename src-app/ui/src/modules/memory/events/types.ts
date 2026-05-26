import type { BaseEvent } from '@/core/events'
import type { UserMemoryRow } from '@/modules/memory/stores/Memories.store'
import type { UserMemorySettingsRow } from '@/modules/memory/stores/MemorySettings.store'
import type { MemoryAdminSettingsRow } from '@/modules/memory/stores/MemoryAdmin.store'

export interface MemoryCreatedEvent extends BaseEvent {
  type: 'memory.created'
  data: { memory: UserMemoryRow }
}
export interface MemoryUpdatedEvent extends BaseEvent {
  type: 'memory.updated'
  data: { memory: UserMemoryRow }
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
  data: { settings: UserMemorySettingsRow }
}
export interface MemoryAdminSettingsUpdatedEvent extends BaseEvent {
  type: 'memory.admin_settings_updated'
  data: { settings: MemoryAdminSettingsRow }
}

export type MemoryModuleEvent =
  | MemoryCreatedEvent
  | MemoryUpdatedEvent
  | MemoryDeletedEvent
  | MemoryAllClearedEvent
  | MemorySettingsUpdatedEvent
  | MemoryAdminSettingsUpdatedEvent

declare module '@/core/events' {
  interface AppEvents {
    'memory.created': MemoryCreatedEvent
    'memory.updated': MemoryUpdatedEvent
    'memory.deleted': MemoryDeletedEvent
    'memory.all_cleared': MemoryAllClearedEvent
    'memory.settings_updated': MemorySettingsUpdatedEvent
    'memory.admin_settings_updated': MemoryAdminSettingsUpdatedEvent
  }
}
