import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Text, Title } from '@ziee/kit'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { Stores } from '@ziee/framework/stores'

export default function NewChatPage() {
  const navigate = useNavigate()

  useEffect(() => {
    Stores.Chat.reset()
    // `/chat` is a SINGLE-PANE surface: collapse the workspace on the way in.
    // Without this, a split left open in the store keeps ConversationPage on its
    // `panes.length >= 2` branch, so the moment this page creates a conversation
    // and navigates to it, the URL→workspace reconcile ("auto while split")
    // REPLACES the focused pane and the old split reappears with the new chat
    // wedged into it — instead of the fresh single conversation that was asked
    // for. Stating the invariant at the ROUTE covers every way in (the sidebar
    // action, the chat-history + onboarding buttons, close-last-pane, a deep
    // link); it is idempotent for the callers that already reset before
    // navigating here.
    //
    // This does NOT touch the in-split "new chat pane" flow: that pane's picker
    // switches to its composer with local state only and never navigates, so
    // this page never mounts and its conversation still adopts into that pane.
    Stores.SplitView.reset()

    const unsubscribe = Stores.EventBus.on(
      'conversation.created',
      event => {
        navigate(`/chat/${event.data.conversation.id}`)
      },
      'NewChatPage',
    )

    return () => {
      unsubscribe()
    }
  }, [navigate])

  return (
    <div className="flex flex-col h-full items-center justify-center p-4">
      <div className="w-full max-w-3xl">
        {/* Welcome message */}
        <div className="text-center mb-12">
          <Title level={2} data-testid="new-chat-greeting">How can I help you today?</Title>
          <Text type="secondary" className="text-lg">
            Start a new conversation by typing a message below
          </Text>
        </div>

        {/* Chat input */}
        <div className="w-full">
          <ChatInput />
        </div>
      </div>
    </div>
  )
}
