import type { StoreProxy } from '@/core/stores'
import type { useUserAssistantsStore, useTemplateAssistantsStore, useAssistantDrawerStore } from './store'

declare module '@/core/stores' {
  interface RegisteredStores {
    UserAssistants: StoreProxy<ReturnType<typeof useUserAssistantsStore.getState>>
    TemplateAssistants: StoreProxy<ReturnType<typeof useTemplateAssistantsStore.getState>>
    AssistantDrawer: StoreProxy<ReturnType<typeof useAssistantDrawerStore.getState>>
  }
}

export {}
