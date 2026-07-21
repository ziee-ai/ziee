import type { StoreProxy } from '@ziee/framework/stores'
import type {
  useUserAssistantsStore,
  useTemplateAssistantsStore,
  useAssistantPickerStore,
} from '@/modules/assistant/stores'
import type { useAssistantDrawerStore } from '@/modules/assistant/components/assistantDrawer'

declare module '@ziee/framework/stores' {
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
