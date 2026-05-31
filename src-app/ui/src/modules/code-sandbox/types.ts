import type { StoreProxy } from '@/core/stores'
import type { useSandboxRootfsVersionsStore } from './stores/SandboxRootfsVersions.store'
import type { useSandboxResourceLimitsStore } from './stores/SandboxResourceLimits.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    SandboxRootfsVersions: StoreProxy<
      ReturnType<typeof useSandboxRootfsVersionsStore.getState>
    >
    SandboxResourceLimits: StoreProxy<
      ReturnType<typeof useSandboxResourceLimitsStore.getState>
    >
  }
}

export {}
