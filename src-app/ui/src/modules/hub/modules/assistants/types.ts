import type { StoreProxy } from '@/core/stores'
import type { useHubAssistantsStore } from '@/modules/hub/modules/assistants/stores/hub-assistants-store'

declare module '@/core/stores' {
  interface RegisteredStores {
    HubAssistants: StoreProxy<ReturnType<typeof useHubAssistantsStore.getState>>
  }
}

export {}
