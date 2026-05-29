import type { StoreProxy } from '@/core/stores'
import type { useRootfsVersionsStore } from './stores/RootfsVersions.store'
import type { useSandboxEnvironmentsStore } from './stores/SandboxEnvironments.store'
import type { useSandboxResourceLimitsStore } from './stores/SandboxResourceLimits.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    RootfsVersions: StoreProxy<
      ReturnType<typeof useRootfsVersionsStore.getState>
    >
    SandboxEnvironments: StoreProxy<
      ReturnType<typeof useSandboxEnvironmentsStore.getState>
    >
    SandboxResourceLimits: StoreProxy<
      ReturnType<typeof useSandboxResourceLimitsStore.getState>
    >
  }
}

export {}
