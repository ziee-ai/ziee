import type { Assistant } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const AssistantDrawer = defineStore('AssistantDrawer', {
  state: {
    open: false,
    loading: false,
    editingAssistant: null as Assistant | null,
    isTemplate: false,
    isCloning: false,
  },
  actions: set => ({
    openAssistantDrawer: (
      assistant?: Assistant | null,
      isTemplate = false,
      isCloning = false,
    ) => set({ open: true, editingAssistant: assistant || null, isTemplate, isCloning }),
    closeAssistantDrawer: () =>
      set({
        open: false,
        loading: false,
        editingAssistant: null,
        isTemplate: false,
        isCloning: false,
      }),
    setAssistantDrawerLoading: (loading: boolean) => set({ loading }),
  }),
  init: ({ on, get, set, actions }) => {
    on('assistant.updated', event => {
      const s = get()
      if (!s.isTemplate && !s.isCloning && s.editingAssistant?.id === event.data.assistant.id) {
        set({ editingAssistant: event.data.assistant })
      }
    })
    on('assistant.deleted', event => {
      const s = get()
      if (!s.isTemplate && !s.isCloning && s.editingAssistant?.id === event.data.assistantId) {
        actions.closeAssistantDrawer()
      }
    })
    on('assistant_template.updated', event => {
      const s = get()
      if (s.isTemplate && !s.isCloning && s.editingAssistant?.id === event.data.template.id) {
        set({ editingAssistant: event.data.template })
      }
    })
    on('assistant_template.deleted', event => {
      const s = get()
      if (s.isTemplate && !s.isCloning && s.editingAssistant?.id === event.data.templateId) {
        actions.closeAssistantDrawer()
      }
    })
  },
})

export const useAssistantDrawerStore = AssistantDrawer.store
