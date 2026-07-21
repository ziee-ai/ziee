import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { McpServersStepGet, McpServersStepSet } from '../state'

export default (set: McpServersStepSet, _get: McpServersStepGet) =>
  async () => {
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
  }
