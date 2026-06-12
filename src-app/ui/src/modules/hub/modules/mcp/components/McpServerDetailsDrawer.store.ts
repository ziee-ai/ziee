import { create } from 'zustand'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type { HubMCPServer } from '@/api-client/types'

interface McpServerDetailsDrawerState {
  isOpen: boolean
  selectedServer: HubMCPServer | null
  /** True while the fresh manifest is being fetched on open. */
  loading: boolean

  // Actions
  open: (server: HubMCPServer) => void
  close: () => void
}

export const useMcpServerDetailsDrawerStore =
  create<McpServerDetailsDrawerState>()(
    immer(
      (set, get): McpServerDetailsDrawerState => ({
        isOpen: false,
        selectedServer: null,
        loading: false,

        // Show the list copy instantly, then refresh from the
        // authoritative current/ manifest via /api/hub/manifest.
        open: (server: HubMCPServer) => {
          set({ isOpen: true, selectedServer: server, loading: true })
          ApiClient.Hub.getManifest({ id: server.name, category: 'mcp-server' })
            .then(manifest => {
              if (
                get().isOpen &&
                get().selectedServer?.name === server.name &&
                manifest.mcp_server
              ) {
                set({ selectedServer: manifest.mcp_server, loading: false })
              } else {
                set({ loading: false })
              }
            })
            .catch(() => set({ loading: false }))
        },

        close: () => {
          set({ isOpen: false, selectedServer: null, loading: false })
        },
      }),
    ),
  )
