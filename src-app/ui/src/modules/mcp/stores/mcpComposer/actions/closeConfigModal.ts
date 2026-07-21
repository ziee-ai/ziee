import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Close the config modal. Clears project scope so reopening from
 * chat doesn't accidentally route via stale state.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => () => {
  set(state => {
    state.configModalVisible = false
    state.currentProjectId = null
  })
}
