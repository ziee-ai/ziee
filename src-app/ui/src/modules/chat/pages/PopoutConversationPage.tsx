import { useParams } from 'react-router-dom'
import ConversationPage from './ConversationPage'
import { usePopoutCloseEmitter } from '@/modules/chat/core/popout/usePopoutSnapBack'

/**
 * The `/chat-window/:conversationId` route element (ITEM-52/54). Renders the exact
 * same `ConversationPage` as `/chat/:id` — but because this route has NO layout it
 * shows WITHOUT the app shell (chat-only, ITEM-52) — and additionally registers the
 * desktop close-emitter so closing the pop-out window snaps the conversation back
 * into the main window as a pane (ITEM-54). On web the emitter is a no-op.
 */
export default function PopoutConversationPage() {
  const { conversationId } = useParams<{ conversationId: string }>()
  usePopoutCloseEmitter(conversationId)
  return <ConversationPage />
}
