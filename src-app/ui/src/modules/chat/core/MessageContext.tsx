import { createContext, useContext } from 'react'
import type { MessageWithContent } from '@/api-client/types'

/**
 * Context that provides the current message to zero-arg slot components.
 * ChatMessage wraps each message's ExtensionSlot with this provider so
 * extensions can access the message without receiving props.
 */
export const MessageContext = createContext<MessageWithContent | null>(null)

export const useMessageContext = (): MessageWithContent | null => {
  return useContext(MessageContext)
}
