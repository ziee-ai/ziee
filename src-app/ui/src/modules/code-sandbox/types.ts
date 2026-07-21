import type { StoreProxy } from '@ziee/framework/stores'
import type { useSandboxRootfsVersionsStore } from './stores/sandboxRootfsVersions'
import type { useSandboxResourceLimitsStore } from './stores/sandboxResourceLimits'

declare module '@ziee/framework/stores' {
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
