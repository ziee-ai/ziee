import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import type { HubMCPServer } from '@/api-client/types'

interface McpServerDetailsDrawerState {
  isOpen: boolean
  selectedServer: HubMCPServer | null

  // Actions
  open: (server: HubMCPServer) => void
  close: () => void
}

export const useMcpServerDetailsDrawerStore = create<McpServerDetailsDrawerState>()(
  immer((set): McpServerDetailsDrawerState => ({
    isOpen: false,
    selectedServer: null,

    open: (server: HubMCPServer) => {
      set({ isOpen: true, selectedServer: server })
    },

    close: () => {
      set({ isOpen: false, selectedServer: null })
    },
  })),
)
