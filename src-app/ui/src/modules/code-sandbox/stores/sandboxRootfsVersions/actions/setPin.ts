import { ApiClient } from '@/api-client'
import type { ActionState, SandboxRootfsVersionsGet, SandboxRootfsVersionsSet } from '../state'

function setAction(s: { actions: Record<string, ActionState> }, key: string, patch: ActionState) {
  const cur = s.actions[key] ?? {}
  s.actions[key] = { ...cur, ...patch }
}

function clearAction(s: { actions: Record<string, ActionState> }, key: string) {
  delete s.actions[key]
}

export default (set: SandboxRootfsVersionsSet, _get: SandboxRootfsVersionsGet) => {
  return async (version: string): Promise<boolean> => {
    const key = `pin::${version}`
    set(s => {
      setAction(s, key, { pinning: true })
      s.error = null
    })
    try {
      const res = await ApiClient.CodeSandbox.setRootfsPin({ version })
      set(s => {
        s.pinnedVersion = res.status.pinned_version ?? null
        s.installed = res.status.installed
        s.available = res.status.available
        // Response carries the full VersionStatus — refresh draining +
        // counts too, or per-row Draining tags stay stale after a swap.
        s.draining = res.status.draining
        s.conversationCount = res.status.conversation_count
        s.mcpServerWorkspaceCount = res.status.mcp_server_workspace_count
        s.hostArch = res.status.host_arch ?? null
        s.hostPackage = res.status.host_package ?? null
        s.availability = res.status.availability
        s.lastSwap = res.swap
      })
      return true
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? `Failed to set pin to ${version}`
      })
      return false
    } finally {
      set(s => {
        clearAction(s, key)
      })
    }
  }
}
