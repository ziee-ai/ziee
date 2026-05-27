import { Stores } from '@/core/stores'
import type {
  MemoryAdminSettings,
  UserMemory,
  UserMemorySettings,
} from '@/api-client/types'

export const emitMemoryCreated = async (memory: UserMemory) => {
  await Stores.EventBus.emit({ type: 'memory.created', data: { memory } })
}
export const emitMemoryUpdated = async (memory: UserMemory) => {
  await Stores.EventBus.emit({ type: 'memory.updated', data: { memory } })
}
export const emitMemoryDeleted = async (memoryId: string) => {
  await Stores.EventBus.emit({ type: 'memory.deleted', data: { memoryId } })
}
export const emitMemoryAllCleared = async (deletedCount: number) => {
  await Stores.EventBus.emit({
    type: 'memory.all_cleared',
    data: { deletedCount },
  })
}
export const emitMemorySettingsUpdated = async (
  settings: UserMemorySettings,
) => {
  await Stores.EventBus.emit({
    type: 'memory.settings_updated',
    data: { settings },
  })
}
export const emitMemoryAdminSettingsUpdated = async (
  settings: MemoryAdminSettings,
) => {
  await Stores.EventBus.emit({
    type: 'memory.admin_settings_updated',
    data: { settings },
  })
}
