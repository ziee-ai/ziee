
import type { McpComposerSet, McpComposerGet } from '../state'
import type { DisabledServer } from '@/api-client/types'

/**
 * Save conversation config changes.
 */
export default (set: McpComposerSet, get: McpComposerGet) => async (
  conversationId: string,
  availableServerIds?: string[],
  serverToolsMap?: Map<string, string[]>,
  updateAutoApproved?: boolean,
) => {
  const state = get()
  const config = state.conversationConfigs.get(conversationId)

  if (!config) {
    console.warn('[MCP Store] No config to save for:', conversationId)
    return
  }

  // Compute disabled_servers from selectedServers (inverted logic)
  let disabledServers: DisabledServer[] = []
  if (availableServerIds && availableServerIds.length > 0) {
    const selectedServerIds = new Set(config.selectedServers.keys())
    disabledServers = availableServerIds
      .filter(id => !selectedServerIds.has(id))
      .map(id => ({ server_id: id, tools: [] }))
  }

  // For partially selected servers (specific tools chosen), compute disabled tools
  if (serverToolsMap) {
    for (const [serverId, selection] of config.selectedServers.entries()) {
      if (selection.tools.length > 0) {
        const allTools = serverToolsMap.get(serverId) || []
        const disabledTools = allTools.filter(t => !selection.tools.includes(t))
        if (disabledTools.length > 0) {
          disabledServers.push({ server_id: serverId, tools: disabledTools })
        }
      }
    }
  }

  // Also include any previously saved disabled servers for unavailable servers
  const existingDisabled = config.disabledServers || []
  const availableSet = new Set(availableServerIds || [])
  const unavailableDisabled = existingDisabled.filter((d: DisabledServer) => !availableSet.has(d.server_id))
  disabledServers = [...disabledServers, ...unavailableDisabled]

  // Call backend API to persist settings
  const { ApiClient } = await import('@/api-client')
  await ApiClient.Conversation.updateMcpSettings({
    id: conversationId,
    approval_mode: config.approvalMode || 'manual_approve',
    ...(updateAutoApproved ? { auto_approved_tools: config.autoApprovedTools } : {}),
    disabled_servers: disabledServers,
    loop_settings: config.loopSettings,
  })

  // Update local state with the computed disabled servers
  set(state => {
    const existingConfig = state.conversationConfigs.get(conversationId)
    if (existingConfig) {
      state.conversationConfigs.set(conversationId, {
        ...existingConfig,
        disabledServers,
      })
    }
  })

  console.log('[MCP Store] Saved conversation config:', conversationId, {
    approvalMode: config.approvalMode,
    autoApprovedTools: config.autoApprovedTools?.length || 0,
    disabledServers: disabledServers.length,
  })
}
