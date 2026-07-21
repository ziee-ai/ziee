import type {
  MemoryAdminSettings,
  UserMemory,
  UserMemorySettings,
} from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export const emitMemoryCreated = async (memory: UserMemory) => {
  await EventBus.emit({ type: 'memory.created', data: { memory } })
}
export const emitMemoryUpdated = async (memory: UserMemory) => {
  await EventBus.emit({ type: 'memory.updated', data: { memory } })
}
export const emitMemoryDeleted = async (memoryId: string) => {
  await EventBus.emit({ type: 'memory.deleted', data: { memoryId } })
}
export const emitMemoryAllCleared = async (deletedCount: number) => {
  await EventBus.emit({
    type: 'memory.all_cleared',
    data: { deletedCount },
  })
}
export const emitMemorySettingsUpdated = async (
  settings: UserMemorySettings,
) => {
  await EventBus.emit({
    type: 'memory.settings_updated',
    data: { settings },
  })
}
export const emitMemoryAdminSettingsUpdated = async (
  settings: MemoryAdminSettings,
) => {
  await EventBus.emit({
    type: 'memory.admin_settings_updated',
    data: { settings },
  })
}
