import { Stores } from '@/core/stores'
import type { UserMemoryRow } from '@/modules/memory/stores/Memories.store'
import type { UserMemorySettingsRow } from '@/modules/memory/stores/MemorySettings.store'
import type { MemoryAdminSettingsRow } from '@/modules/memory/stores/MemoryAdmin.store'

export const emitMemoryCreated = async (memory: UserMemoryRow) => {
  await Stores.EventBus.emit({ type: 'memory.created', data: { memory } })
}
export const emitMemoryUpdated = async (memory: UserMemoryRow) => {
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
export const emitMemorySettingsUpdated = async (settings: UserMemorySettingsRow) => {
  await Stores.EventBus.emit({
    type: 'memory.settings_updated',
    data: { settings },
  })
}
export const emitMemoryAdminSettingsUpdated = async (settings: MemoryAdminSettingsRow) => {
  await Stores.EventBus.emit({
    type: 'memory.admin_settings_updated',
    data: { settings },
  })
}
