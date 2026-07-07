import { createContext, useContext } from 'react'

/**
 * ConversationFindContext (ITEM-1) — carries the id of the message that is the
 * ACTIVE find match so `ChatMessage` can highlight it, without prop-drilling
 * through `MessageList`. `null` when the find bar is closed or has no matches.
 */
export const ConversationFindContext = createContext<{
  activeMatchId: string | null
}>({ activeMatchId: null })

export function useConversationFind() {
  return useContext(ConversationFindContext)
}
