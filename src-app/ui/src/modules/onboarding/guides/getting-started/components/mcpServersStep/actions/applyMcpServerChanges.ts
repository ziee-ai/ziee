import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { McpServersStepGet, McpServersStepSet } from '../state'

export default (set: McpServersStepSet, get: McpServersStepGet) =>
  async () => {
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
  }
