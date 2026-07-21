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
  return async (id: string): Promise<boolean> => {
    const key = `del::${id}`
    set(s => {
      setAction(s, key, { deleting: true })
      s.error = null
    })
    try {
      const res = await ApiClient.CodeSandbox.deleteRootfsVersion({ id })
      set(s => {
        s.pinnedVersion = res.pinned_version ?? null
        s.installed = res.installed
        s.available = res.available
        s.draining = res.draining
        s.conversationCount = res.conversation_count
        s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
        s.hostArch = res.host_arch ?? null
        s.hostPackage = res.host_package ?? null
        s.availability = res.availability
      })
      return true
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to delete artifact'
      })
      return false
    } finally {
      set(s => {
        clearAction(s, key)
      })
    }
  }
}
