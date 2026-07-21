import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { assistantDrawerState, type AssistantDrawerState } from './state'
import type { Actions } from './actions.gen'

const AssistantDrawerDef = defineStore<AssistantDrawerState, Actions>('AssistantDrawer', {
  state: assistantDrawerState,
  actions: import.meta.glob('./actions/*.ts'),
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

export const AssistantDrawer = registerLazyStore(AssistantDrawerDef)
export const useAssistantDrawerStore = AssistantDrawerDef.store
