import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import type { HubModel } from '@/api-client/types'

interface ModelDetailsDrawerState {
  isOpen: boolean
  selectedModel: HubModel | null

  // Actions
  open: (model: HubModel) => void
  close: () => void
}

export const useModelDetailsDrawerStore = create<ModelDetailsDrawerState>()(
  immer((set): ModelDetailsDrawerState => ({
    isOpen: false,
    selectedModel: null,

    open: (model: HubModel) => {
      set({ isOpen: true, selectedModel: model })
    },

    close: () => {
      set({ isOpen: false, selectedModel: null })
    },
  })),
)
