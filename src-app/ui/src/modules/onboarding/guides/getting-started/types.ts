import type { StoreProxy } from '@/core/stores'
import type { useApiKeysStepStore } from './components/ApiKeysStep.store'
import type { useMcpServersStepStore } from './components/McpServersStep.store'
import type { useMemorySetupStepStore } from './components/MemorySetupStep.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    ApiKeysStep: StoreProxy<ReturnType<typeof useApiKeysStepStore.getState>>
    McpServersStep: StoreProxy<ReturnType<typeof useMcpServersStepStore.getState>>
    MemorySetupStep: StoreProxy<ReturnType<typeof useMemorySetupStepStore.getState>>
  }
}

export {}
