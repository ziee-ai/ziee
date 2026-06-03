import type { StoreProxy } from '@/core/stores'
import type {
  useUserAssistantsStore,
  useTemplateAssistantsStore,
  useAssistantPickerStore,
} from '@/modules/assistant/stores'
import type { useAssistantDrawerStore } from '@/modules/assistant/components/AssistantDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    UserAssistants: StoreProxy<
      ReturnType<typeof useUserAssistantsStore.getState>
    >
    TemplateAssistants: StoreProxy<
      ReturnType<typeof useTemplateAssistantsStore.getState>
    >
    AssistantDrawer: StoreProxy<
      ReturnType<typeof useAssistantDrawerStore.getState>
    >
    AssistantPicker: StoreProxy<
      ReturnType<typeof useAssistantPickerStore.getState>
    >
  }
}

export {}
