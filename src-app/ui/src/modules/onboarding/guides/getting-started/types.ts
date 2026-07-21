import type { StoreProxy } from '@ziee/framework/stores'
import type { useApiKeysStepStore } from './components/ApiKeysStep.store'
import type { useMcpServersStepStore } from './components/mcpServersStep'
import type { useMemorySetupStepStore } from './components/MemorySetupStep.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    ApiKeysStep: StoreProxy<ReturnType<typeof useApiKeysStepStore.getState>>
    McpServersStep: StoreProxy<ReturnType<typeof useMcpServersStepStore.getState>>
    MemorySetupStep: StoreProxy<ReturnType<typeof useMemorySetupStepStore.getState>>
  }
}

export {}
