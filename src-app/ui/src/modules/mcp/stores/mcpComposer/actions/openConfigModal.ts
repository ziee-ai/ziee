import type { McpComposerSet, McpComposerGet } from '../state'

/**
 * Open the config modal.
 */
export default (set: McpComposerSet, _get: McpComposerGet) => () => {
  set(state => {
    // Conversation-scoped open: clear any stale project scope so
    // the dispatch rule routes the save to the conversation path.
    state.currentProjectId = null
    state.configModalVisible = true
  })
}
