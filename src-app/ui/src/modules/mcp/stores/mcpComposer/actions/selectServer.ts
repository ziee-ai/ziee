import { pendingConversationKey } from '../../approvalRouting'
import { resolveConfigKey } from '../state'
import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Select a server (tools=[] means all tools).
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (
  serverId: string,
  tools: string[] = [],
  paneId?: string | null,
) => {
  set(state => {
    state.selectedServers.set(serverId, {
      server_id: serverId,
      tools,
    })

    // Update the target config. When an explicit `paneId` is given (ITEM-51
    // new-chat seeding from McpInitializer for a specific pane), target THAT
    // pane's OWN pending config directly, so a new SPLIT pane's default servers
    // don't land in the current-scope/other-pane config. Otherwise (the modal)
    // use the current scope's key (which is already currentPaneId-aware).
    const configKey =
      paneId !== undefined
        ? pendingConversationKey(paneId)
        : resolveConfigKey(state, state.currentConversationId)
    let config = state.conversationConfigs.get(configKey)
    // Seeding a specific pane's pending config (paneId given) may run before
    // that config exists — create an empty one so the selection isn't dropped.
    if (!config && paneId !== undefined) {
      config = { selectedServers: new Map(), approvalMode: 'manual_approve', autoApprovedTools: [] }
      state.conversationConfigs.set(configKey, config)
    }
    if (config) {
      config.selectedServers.set(serverId, { server_id: serverId, tools })
    }
  })
  console.log('[MCP Store] Selected server:', serverId, 'tools:', tools)
}
