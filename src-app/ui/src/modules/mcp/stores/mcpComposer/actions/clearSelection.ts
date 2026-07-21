import type { McpComposerSet, McpComposerGet } from '../state'

/** Clear all server selections. */
export default (set: McpComposerSet, _get: McpComposerGet) => () => {
  set(state => {
    state.selectedServers.clear()
  })
  console.log('[MCP Store] Cleared all server selections')
}
