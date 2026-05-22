import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { createStoreProxy } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { McpServer, HubMCPServer } from '@/api-client/types'

interface McpServersStepStore {
  selectedMcpServerIds: string[]
  systemServers: McpServer[]
  hubServers: HubMCPServer[]
  installedNames: Set<string>
  disabledSystemIds: Set<string>
  loadingServers: boolean
  serversError: string | null

  toggleMcpServer: (id: string) => void
  loadMcpServers: () => Promise<void>
  toggleSystemServer: (id: string, enabled: boolean) => void
  installSelectedMcpServers: () => Promise<void>
  reset: () => void
}

export const useMcpServersStepStore = create<McpServersStepStore>()(
  subscribeWithSelector(
    immer((set, get) => ({
      selectedMcpServerIds: [],
      systemServers: [],
      hubServers: [],
      installedNames: new Set<string>(),
      disabledSystemIds: new Set<string>(),
      loadingServers: false,
      serversError: null,

      toggleMcpServer: (id: string) => {
        set(draft => {
          const idx = draft.selectedMcpServerIds.indexOf(id)
          if (idx >= 0) {
            draft.selectedMcpServerIds.splice(idx, 1)
          } else {
            draft.selectedMcpServerIds.push(id)
          }
        })
      },

      loadMcpServers: async () => {
        set(state => {
          state.loadingServers = true
          state.serversError = null
        })
        try {
          const [mcpResponse, hubResponse] = await Promise.all([
            ApiClient.McpServer.listAccessible({ page: 1, per_page: 50 }, undefined),
            ApiClient.Hub.getMCPServers({}, undefined),
          ])
          set(state => {
            state.systemServers = mcpResponse.servers.filter(s => s.is_system)
            state.installedNames = new Set(mcpResponse.servers.map(s => s.name))
            state.hubServers = hubResponse
            state.loadingServers = false
          })
        } catch (error: any) {
          set(state => {
            state.serversError = error.message || 'Failed to load servers'
            state.loadingServers = false
          })
        }
      },

      toggleSystemServer: (id: string, enabled: boolean) => {
        set(draft => {
          if (enabled) {
            draft.disabledSystemIds.delete(id)
          } else {
            draft.disabledSystemIds.add(id)
          }
        })
      },

      installSelectedMcpServers: async () => {
        const { selectedMcpServerIds } = get()
        for (const hubId of selectedMcpServerIds) {
          await ApiClient.Hub.createMcpServerFromHub(
            { hub_id: hubId, enabled: true },
            undefined,
          )
        }
      },

      reset: () => {
        set(draft => {
          draft.selectedMcpServerIds = []
          draft.systemServers = []
          draft.hubServers = []
          draft.installedNames = new Set()
          draft.loadingServers = false
          draft.serversError = null
          draft.disabledSystemIds = new Set()
        })
      },
    })),
  ),
)

export const McpServersStepStoreProxy = createStoreProxy(useMcpServersStepStore)
