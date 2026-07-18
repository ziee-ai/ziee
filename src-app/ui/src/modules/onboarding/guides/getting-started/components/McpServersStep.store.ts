import { ApiClient } from '@/api-client'
import { type HubMCPServer, type McpServer, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'
import { createStoreProxy } from '@ziee/framework/stores'

export const McpServersStep = defineStore('McpServersStep', {
  immer: true,
  state: {
    selectedMcpServerIds: [] as string[],
    systemServers: [] as McpServer[],
    hubServers: [] as HubMCPServer[],
    installedNames: new Set<string>(),
    /** IDs of system servers the user wants DISABLED */
    disabledSystemIds: new Set<string>(),
    /** Snapshot of disabledSystemIds at load — used to compute the diff on apply */
    originalDisabledSystemIds: new Set<string>(),
    loadingServers: false,
    serversError: null as string | null,
  },
  actions: (set, get) => ({
    toggleMcpServer: (id: string) => {
      set(draft => {
        const idx = draft.selectedMcpServerIds.indexOf(id)
        if (idx >= 0) draft.selectedMcpServerIds.splice(idx, 1)
        else draft.selectedMcpServerIds.push(id)
      })
    },
    loadMcpServers: async () => {
      set(state => {
        state.loadingServers = true
        state.serversError = null
      })
      try {
        // An admin (McpServersAdminEdit) manages ALL system servers here, not
        // just their group-assigned ones. Source from the admin list when the
        // user can manage; non-admins fall back to the accessible list.
        const canManage = hasPermissionNow(Permissions.McpServersAdminEdit)
        const [mcpResponse, hubResponse, systemResponse] = await Promise.all([
          ApiClient.McpServer.listAccessible({ page: 1, per_page: 50 }, undefined),
          ApiClient.Hub.getMCPServers({}, undefined),
          canManage
            ? ApiClient.McpServerSystem.list({ page: 1, per_page: 50 }, undefined)
            : Promise.resolve(null),
        ])
        set(state => {
          const systemServers =
            canManage && systemResponse
              ? systemResponse.servers
              : mcpResponse.servers.filter(s => s.is_system)
          state.systemServers = systemServers
          state.installedNames = new Set(mcpResponse.servers.map(s => s.name))
          state.hubServers = hubResponse
          const disabled = new Set<string>()
          for (const s of systemServers) if (!s.enabled) disabled.add(s.id)
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
        if (enabled) draft.disabledSystemIds.delete(id)
        else draft.disabledSystemIds.add(id)
      })
    },
    // Persist hub-server installs AND system-server toggles; collect per-item
    // errors and throw once at the end so the wizard error UI shows the batch.
    applyMcpServerChanges: async () => {
      const {
        selectedMcpServerIds,
        disabledSystemIds,
        originalDisabledSystemIds,
        systemServers,
        installedNames,
      } = get()
      const errors: string[] = []
      const newlyInstalled: string[] = []

      // 1. Install hub servers — skip already-installed (idempotent so both
      //    McpServersStep and FinishStep can register this handler).
      for (const hubId of selectedMcpServerIds) {
        if (installedNames.has(hubId)) continue
        try {
          await ApiClient.Hub.createMcpServerFromHub({ hub_id: hubId }, undefined)
          newlyInstalled.push(hubId)
        } catch (err: any) {
          errors.push(`Install "${hubId}": ${err.message || 'Unknown error'}`)
        }
      }
      if (newlyInstalled.length > 0) {
        set(draft => {
          for (const name of newlyInstalled) draft.installedNames.add(name)
        })
      }

      // 2. Persist system-server toggles (only the changed ones). Use the admin
      //    endpoint when the user can manage — matching the source + the global
      //    enable/disable semantics of a system server.
      const canManage = hasPermissionNow(Permissions.McpServersAdminEdit)
      for (const server of systemServers) {
        const wantsDisabled = disabledSystemIds.has(server.id)
        const wasDisabled = originalDisabledSystemIds.has(server.id)
        if (wantsDisabled === wasDisabled) continue
        try {
          if (canManage) {
            await ApiClient.McpServerSystem.update(
              { id: server.id, enabled: !wantsDisabled },
              undefined,
            )
          } else {
            await ApiClient.McpServer.update(
              { id: server.id, enabled: !wantsDisabled },
              undefined,
            )
          }
        } catch (err: any) {
          errors.push(
            `Toggle "${server.display_name || server.name}": ${err.message || 'Unknown error'}`,
          )
        }
      }

      if (errors.length > 0) {
        throw new Error(`Failed to apply MCP server changes: ${errors.join('; ')}`)
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
  }),
})

export const useMcpServersStepStore = McpServersStep.store
export const McpServersStepStoreProxy = createStoreProxy(useMcpServersStepStore)
