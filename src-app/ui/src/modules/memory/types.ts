import type { StoreProxy } from '@ziee/framework/stores'
import type { useMemoriesStore } from './stores/memories'
import type { useMemorySettingsStore } from './stores/MemorySettings.store'
import type { useMemoryAdminStore } from './stores/memoryAdmin'
import type { useMemoryAuditStore } from './stores/memoryAudit'
import type { useCoreMemoryBlocksStore } from './stores/coreMemoryBlocks'

declare module '@ziee/framework/stores' {
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
