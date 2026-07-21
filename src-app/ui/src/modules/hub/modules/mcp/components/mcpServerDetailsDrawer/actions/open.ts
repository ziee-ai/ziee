import { ApiClient } from '@/api-client'
import type { HubMCPServer } from '@/api-client/types'
import type { McpServerDetailsDrawerSet, McpServerDetailsDrawerGet } from '../state'

export default function openFactory(set: McpServerDetailsDrawerSet, get: McpServerDetailsDrawerGet) {
  return async (server: HubMCPServer) => {
    set({ isOpen: true, selectedServer: server, loading: true })
    try {
      const manifest = await ApiClient.Hub.getManifest({ id: server.name, category: 'mcp-server' })
      if (get().isOpen && get().selectedServer?.name === server.name && manifest.mcp_server) {
        set({ selectedServer: manifest.mcp_server, loading: false })
      } else {
        set({ loading: false })
      }
    } catch {
      set({ loading: false })
    }
  }
}
