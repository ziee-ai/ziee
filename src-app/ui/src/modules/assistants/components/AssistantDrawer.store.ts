import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import type { Assistant } from '@/api-client/types'
import { Stores } from '@/core/stores'

interface AssistantDrawerState {
  open: boolean
  loading: boolean
  editingAssistant: Assistant | null
  isTemplate: boolean
  isCloning: boolean

  // Actions
  openAssistantDrawer: (assistant?: Assistant | null, isTemplate?: boolean, isCloning?: boolean) => void
  closeAssistantDrawer: () => void
  setAssistantDrawerLoading: (loading: boolean) => void

  // Initialization
  __init__: {
    __store__: () => void
  }
}

export const useAssistantDrawerStore = create<AssistantDrawerState>()(
  subscribeWithSelector(
    (set, get): AssistantDrawerState => ({
      open: false,
      loading: false,
      editingAssistant: null,
      isTemplate: false,
      isCloning: false,

      __init__: {
        __store__: () => {
          const eventBus = Stores.EventBus

          // Subscribe to assistant.updated (user assistants)
          eventBus.on('assistant.updated', async event => {
            const { assistant } = event.data
            const state = get()

            if (!state.isTemplate && !state.isCloning && state.editingAssistant?.id === assistant.id) {
              set({ editingAssistant: assistant })
            }
          })

          // Subscribe to assistant.deleted (user assistants)
          eventBus.on('assistant.deleted', async event => {
            const { assistantId } = event.data
            const state = get()

            if (!state.isTemplate && !state.isCloning && state.editingAssistant?.id === assistantId) {
              get().closeAssistantDrawer()
            }
          })

          // Subscribe to assistant_template.updated (template assistants)
          eventBus.on('assistant_template.updated', async event => {
            const { template } = event.data
            const state = get()

            if (state.isTemplate && !state.isCloning && state.editingAssistant?.id === template.id) {
              set({ editingAssistant: template })
            }
          })

          // Subscribe to assistant_template.deleted (template assistants)
          eventBus.on('assistant_template.deleted', async event => {
            const { templateId } = event.data
            const state = get()

            if (state.isTemplate && !state.isCloning && state.editingAssistant?.id === templateId) {
              get().closeAssistantDrawer()
            }
          })
        },
      },

      // Actions
      openAssistantDrawer: (assistant?: Assistant | null, isTemplate = false, isCloning = false) => {
        set({
          open: true,
          editingAssistant: assistant || null,
          isTemplate,
          isCloning,
        })
      },

      closeAssistantDrawer: () => {
        set({
          open: false,
          loading: false,
          editingAssistant: null,
          isTemplate: false,
          isCloning: false,
        })
      },

      setAssistantDrawerLoading: (loading: boolean) => {
        set({ loading })
      },
    }),
  ),
)
