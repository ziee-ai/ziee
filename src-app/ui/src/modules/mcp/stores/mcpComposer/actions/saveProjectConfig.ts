import { projectConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'
import type { DisabledServer } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

/**
 * Save the project's MCP defaults. Mirrors saveConversationConfig
 * but targets PUT /projects/{id}/mcp-settings.
 */
export default (set: McpComposerSet, get: McpComposerGet) => async (
  projectId: string,
  availableServerIds?: string[],
  serverToolsMap?: Map<string, string[]>,
) => {
  const key = projectConfigKey(projectId)
  const state = get()
  const config = state.conversationConfigs.get(key)
  if (!config) {
    console.warn('[MCP Store] No project config to save for:', projectId)
    return
  }

  // Disabled-server derivation, identical to saveConversationConfig.
  let disabledServers: DisabledServer[] = []
  if (availableServerIds && availableServerIds.length > 0) {
    const selectedServerIds = new Set(config.selectedServers.keys())
    disabledServers = availableServerIds
      .filter(id => !selectedServerIds.has(id))
      .map(id => ({ server_id: id, tools: [] }))
  }
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
  const existingDisabled = config.disabledServers || []
  const availableSet = new Set(availableServerIds || [])
  const unavailableDisabled = existingDisabled.filter((d: DisabledServer) => !availableSet.has(d.server_id))
  disabledServers = [...disabledServers, ...unavailableDisabled]

  const { ApiClient } = await import('@/api-client')
  await ApiClient.Project.updateMcpSettings({
    id: projectId,
    approval_mode: config.approvalMode || 'manual_approve',
    auto_approved_tools: config.autoApprovedTools || [],
    disabled_servers: disabledServers,
    loop_settings: config.loopSettings,
  })

  set(state => {
    const existing = state.conversationConfigs.get(key)
    if (existing) {
      state.conversationConfigs.set(key, { ...existing, disabledServers })
    }
  })

  // Fire `project.mcp_updated` so the dedicated ProjectMcpSettings
  // store (used by the project panel) refetches and the UI reflects
  // the new defaults. Dynamic import to avoid module cycle.
  await EventBus.emit({
    type: 'project.mcp_updated',
    data: { projectId },
  })

  console.log('[MCP Store] Saved project config:', projectId, {
    approvalMode: config.approvalMode,
    autoApprovedTools: config.autoApprovedTools?.length || 0,
    disabledServers: disabledServers.length,
  })
}
