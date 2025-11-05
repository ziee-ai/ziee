import { create } from 'zustand'
import type { Assistant } from '@/api-client/types'

interface AssistantDrawerState {
  open: boolean
  loading: boolean
  editingAssistant: Assistant | null
  isTemplate: boolean
}

export const useAssistantDrawerStore = create<AssistantDrawerState>(() => ({
  open: false,
  loading: false,
  editingAssistant: null,
  isTemplate: false,
}))

// Drawer actions
export const openAssistantDrawer = (
  assistant?: Assistant | null,
  isTemplate = false,
) => {
  useAssistantDrawerStore.setState({
    open: true,
    editingAssistant: assistant || null,
    isTemplate,
  })
}

export const closeAssistantDrawer = () => {
  useAssistantDrawerStore.setState({
    open: false,
    loading: false,
    editingAssistant: null,
    isTemplate: false,
  })
}

export const setAssistantDrawerLoading = (loading: boolean) => {
  useAssistantDrawerStore.setState({
    loading,
  })
}
