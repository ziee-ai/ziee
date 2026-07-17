import type { StoreProxy } from '@ziee/framework/stores'
import type { useAgentAdminSettingsStore } from './stores/AgentAdminSettings.store'

declare module '@ziee/framework/stores' {
  interface RegisteredStores {
    AgentAdminSettings: StoreProxy<
      ReturnType<typeof useAgentAdminSettingsStore.getState>
    >
  }
}

export {}
