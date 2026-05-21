import type { StoreProxy } from '@/core/stores'
import type { useSandboxEnvironmentsStore } from './stores/SandboxEnvironments.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    SandboxEnvironments: StoreProxy<
      ReturnType<typeof useSandboxEnvironmentsStore.getState>
    >
  }
}

export {}
