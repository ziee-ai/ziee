import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Set enabled servers from a list of IDs.
 * Deselects all current servers, then selects only the provided IDs.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => (serverIds: string[]) => {
  set(state => {
    state.selectedServers.clear()
    for (const serverId of serverIds) {
      state.selectedServers.set(serverId, { server_id: serverId, tools: [] })
    }
  })
  console.log('[MCP Store] Set enabled servers:', serverIds)
}
