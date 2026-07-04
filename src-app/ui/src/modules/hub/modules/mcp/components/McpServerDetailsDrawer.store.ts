import { ApiClient } from '@/api-client'
import type { HubMCPServer } from '@/api-client/types'
import { defineStore } from '@/core/store-kit'

export const McpServerDetailsDrawer = defineStore('McpServerDetailsDrawer', {
  immer: true,
  state: {
    isOpen: false,
    selectedServer: null as HubMCPServer | null,
    /** True while the fresh manifest is being fetched on open. */
    loading: false,
  },
  actions: (set, get) => ({
    // Show the list copy instantly, then refresh from the authoritative
    // current/ manifest via /api/hub/manifest.
    open: (server: HubMCPServer) => {
      set({ isOpen: true, selectedServer: server, loading: true })
      ApiClient.Hub.getManifest({ id: server.name, category: 'mcp-server' })
        .then(manifest => {
          if (get().isOpen && get().selectedServer?.name === server.name && manifest.mcp_server) {
            set({ selectedServer: manifest.mcp_server, loading: false })
          } else {
            set({ loading: false })
          }
        })
        .catch(() => set({ loading: false }))
    },
    close: () => set({ isOpen: false, selectedServer: null, loading: false }),
  }),
})

export const useMcpServerDetailsDrawerStore = McpServerDetailsDrawer.store
