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
  /** IDs of system servers the user wants DISABLED */
  disabledSystemIds: Set<string>
  /** Snapshot of disabledSystemIds when servers were loaded — used to compute the diff on apply */
  originalDisabledSystemIds: Set<string>
  loadingServers: boolean
  serversError: string | null

  toggleMcpServer: (id: string) => void
  loadMcpServers: () => Promise<void>
  toggleSystemServer: (id: string, enabled: boolean) => void
  /** Persist hub-server installations AND system-server toggles.
   *  Collects per-item errors and throws once at the end so the wizard
   *  error UI surfaces the full batch result. */
  applyMcpServerChanges: () => Promise<void>
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
      originalDisabledSystemIds: new Set<string>(),
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
            const systemServers = mcpResponse.servers.filter(s => s.is_system)
            state.systemServers = systemServers
            state.installedNames = new Set(mcpResponse.servers.map(s => s.name))
            state.hubServers = hubResponse
            // Initialise disabled-sets from the current server state
            const disabled = new Set<string>()
            for (const s of systemServers) {
              if (!s.enabled) disabled.add(s.id)
            }
            state.disabledSystemIds = disabled
            state.originalDisabledSystemIds = new Set(disabled)
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

      applyMcpServerChanges: async () => {
        const { selectedMcpServerIds, disabledSystemIds, originalDisabledSystemIds, systemServers, installedNames } = get()
        const errors: string[] = []
        const newlyInstalled: string[] = []

        // 1. Install hub servers — skip servers already installed (idempotent
        //    so both McpServersStep and FinishStep can register this handler)
        for (const hubId of selectedMcpServerIds) {
          if (installedNames.has(hubId)) continue
          try {
            await ApiClient.Hub.createMcpServerFromHub(
              { hub_id: hubId, enabled: true },
              undefined,
            )
            newlyInstalled.push(hubId)
          } catch (err: any) {
            errors.push(`Install "${hubId}": ${err.message || 'Unknown error'}`)
          }
        }
        // Update installedNames so a second call from FinishStep is a no-op
        if (newlyInstalled.length > 0) {
          set(draft => {
            for (const name of newlyInstalled) {
              draft.installedNames.add(name)
            }
          })
        }

        // 2. Persist system-server toggles (only the ones that changed)
        for (const server of systemServers) {
          const wantsDisabled = disabledSystemIds.has(server.id)
          const wasDisabled = originalDisabledSystemIds.has(server.id)
          if (wantsDisabled === wasDisabled) continue

          try {
            await ApiClient.McpServer.update(
              { id: server.id, enabled: !wantsDisabled },
              undefined,
            )
          } catch (err: any) {
            errors.push(`Toggle "${server.display_name || server.name}": ${err.message || 'Unknown error'}`)
          }
        }

        if (errors.length > 0) {
          throw new Error(
            `Failed to apply MCP server changes: ${errors.join('; ')}`,
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
          draft.originalDisabledSystemIds = new Set()
        })
      },
    })),
  ),
)

export const McpServersStepStoreProxy = createStoreProxy(useMcpServersStepStore)
