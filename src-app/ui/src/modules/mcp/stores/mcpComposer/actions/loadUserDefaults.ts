import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Load user defaults from backend.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => async () => {
  // Permission-gate the shell-eager-load fetch:
  if (!hasPermissionNow(Permissions.ConversationsRead)) return

  try {
    const { ApiClient } = await import('@/api-client')
    const response = await ApiClient.Mcp.getDefaults()
    set(state => {
      state.userDefaults = response.defaults || null
      state.userDefaultsLoaded = true
    })
    console.log('[MCP Store] Loaded user defaults:', response.defaults)
  } catch (error) {
    console.error('[MCP Store] Failed to load user defaults:', error)
    set(state => {
      state.userDefaultsLoaded = true
    })
  }
}
