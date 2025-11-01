import { create } from 'zustand'
import type { LlmProvider } from '@/api-client/types'

interface LlmProviderDrawerState {
  isOpen: boolean
  editingProvider: LlmProvider | null
}

export const useLlmProviderDrawerStore = create<LlmProviderDrawerState>(() => ({
  isOpen: false,
  editingProvider: null,
}))

export const openLlmProviderDrawer = (provider?: LlmProvider) => {
  useLlmProviderDrawerStore.setState({ isOpen: true, editingProvider: provider ?? null })
}

export const closeLlmProviderDrawer = () => {
  useLlmProviderDrawerStore.setState({ isOpen: false, editingProvider: null })
}
