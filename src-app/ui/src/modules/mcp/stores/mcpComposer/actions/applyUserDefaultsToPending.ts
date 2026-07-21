import { pendingConversationKey } from '../../approvalRouting'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Apply user defaults to pending config (for new conversations).
 */
export default (set: McpComposerSet, get: McpComposerGet) => (
  availableServerIds: string[],
  paneId?: string | null,
) => {
  const state = get()
  const defaults = state.userDefaults

  if (!defaults) {
    console.log('[MCP Store] No user defaults to apply')
    return
  }

  // Compute selected servers from disabled_servers
  // All available servers are selected EXCEPT those in disabled_servers
  const disabledServerIds = new Set((defaults.disabled_servers || []).map(d => d.server_id))
  const selectedServers = new Map<string, { server_id: string; tools: string[] }>()

  for (const serverId of availableServerIds) {
    if (!disabledServerIds.has(serverId)) {
      selectedServers.set(serverId, { server_id: serverId, tools: [] })
    }
  }

  set(s => {
    // THIS pane's pending config (ITEM-51) so a new SPLIT pane's default MCP
    // servers land in the same per-pane key its status row + send path read
    // (a null pane → the bare key, single-pane unchanged).
    s.conversationConfigs.set(pendingConversationKey(paneId), {
      selectedServers,
      disabledServers: defaults.disabled_servers || [],
      approvalMode: defaults.approval_mode as 'disabled' | 'auto_approve' | 'manual_approve',
      autoApprovedTools: defaults.auto_approved_tools || [],
      loopSettings: defaults.loop_settings,
    })
    // Also update the active projection when this pane's default IS the active
    // scope (a new conversation whose current pane is this one).
    if (!s.currentConversationId && (s.currentPaneId ?? null) === (paneId ?? null)) {
      s.selectedServers = new Map(selectedServers)
    }
  })
  console.log('[MCP Store] Applied user defaults to pending config:', {
    selectedServers: selectedServers.size,
    approvalMode: defaults.approval_mode,
  })
}
