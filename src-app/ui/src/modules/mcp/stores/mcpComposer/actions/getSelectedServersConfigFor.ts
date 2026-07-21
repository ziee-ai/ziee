import { pendingConversationKey } from '../../approvalRouting'
import type { McpComposerGet } from '../state'
import type { McpServerConfig } from '@/api-client/types'

/**
 * The selected-servers config for a SPECIFIC conversation (ITEM-33) —
 * resolved from the per-conversation `conversationConfigs` (keyed), NOT the
 * single-active `selectedServers`. Synchronous.
 */
export default (_set: unknown, get: McpComposerGet): (
  conversationId: string | null | undefined,
  paneId?: string | null,
) => McpServerConfig[] => {
  return (
    conversationId: string | null | undefined,
    paneId?: string | null,
  ): McpServerConfig[] => {
    const state = get()
    // For a new-chat (pre-mint) pane read THIS pane's OWN pending config
    // (ITEM-51) — the send path is the READ twin of the pane-aware pending
    // WRITE. A pending pane with no stored config has NO selection: NEVER fall
    // back to the global `selectedServers` projection there (that is whichever
    // pane last opened its modal, so it would leak the OTHER pane's servers or,
    // when empty, silently drop this pane's). The active-projection fallback is
    // ONLY for the same-REAL-conversation just-created case.
    const key = conversationId ?? pendingConversationKey(paneId)
    const config = state.conversationConfigs.get(key)
    const selections = config
      ? Array.from(config.selectedServers.values())
      : conversationId != null && conversationId === state.currentConversationId
        ? Array.from(state.selectedServers.values())
        : []
    return selections.map(sel => ({
      server_id: sel.server_id,
      tools: sel.tools.length > 0 ? sel.tools : undefined,
    }))
  }
}
