import { create } from 'zustand'
import type { Assistant } from '@/api-client/types'

interface AssistantDrawerState {
  open: boolean
  loading: boolean
  editingAssistant: Assistant | null
  isTemplate: boolean

  // Actions
  openAssistantDrawer: (assistant?: Assistant | null, isTemplate?: boolean) => void
  closeAssistantDrawer: () => void
  setAssistantDrawerLoading: (loading: boolean) => void
}

export const useAssistantDrawerStore = create<AssistantDrawerState>(
  (set): AssistantDrawerState => ({
    open: false,
    loading: false,
    editingAssistant: null,
    isTemplate: false,

    // Actions
    openAssistantDrawer: (assistant?: Assistant | null, isTemplate = false) => {
      set({
        open: true,
        editingAssistant: assistant || null,
        isTemplate,
      })
    },

    closeAssistantDrawer: () => {
      set({
        open: false,
        loading: false,
        editingAssistant: null,
        isTemplate: false,
      })
    },

    setAssistantDrawerLoading: (loading: boolean) => {
      set({ loading })
    },
  }),
)
