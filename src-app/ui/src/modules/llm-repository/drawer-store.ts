import { create } from 'zustand'
import type { LlmRepository } from '@/api-client/types'

interface LlmRepositoryDrawerState {
  open: boolean
  loading: boolean
  editingRepository: LlmRepository | null
}

export const useLlmRepositoryDrawerStore = create<LlmRepositoryDrawerState>(() => ({
  open: false,
  loading: false,
  editingRepository: null,
}))

// Modal actions
export const openLlmRepositoryDrawer = (repository?: LlmRepository) => {
  useLlmRepositoryDrawerStore.setState({
    open: true,
    editingRepository: repository || null,
  })
}

export const closeLlmRepositoryDrawer = () => {
  useLlmRepositoryDrawerStore.setState({
    open: false,
    loading: false,
    editingRepository: null,
  })
}

export const setLlmRepositoryDrawerLoading = (loading: boolean) => {
  useLlmRepositoryDrawerStore.setState({
    loading,
  })
}
