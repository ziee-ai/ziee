import type { StoreProxy } from '@/core/stores'
import type { useMemoriesStore } from './stores/Memories.store'
import type { useMemorySettingsStore } from './stores/MemorySettings.store'
import type { useMemoryAdminStore } from './stores/MemoryAdmin.store'
import type { useMemoryAuditStore } from './stores/MemoryAudit.store'
import type { useCoreMemoryBlocksStore } from './stores/CoreMemoryBlocks.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    Memories: StoreProxy<ReturnType<typeof useMemoriesStore.getState>>
    MemorySettings: StoreProxy<
      ReturnType<typeof useMemorySettingsStore.getState>
    >
    MemoryAdmin: StoreProxy<ReturnType<typeof useMemoryAdminStore.getState>>
    MemoryAudit: StoreProxy<ReturnType<typeof useMemoryAuditStore.getState>>
    CoreMemoryBlocks: StoreProxy<
      ReturnType<typeof useCoreMemoryBlocksStore.getState>
    >
  }
}

export {}
