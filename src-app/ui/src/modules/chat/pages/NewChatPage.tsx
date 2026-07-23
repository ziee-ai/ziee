import { useEffect } from 'react'
import { useNavigate } from 'react-router-dom'
import { Text, Title } from '@ziee/kit'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { Stores } from '@ziee/framework/stores'

export default function NewChatPage() {
  const navigate = useNavigate()

  useEffect(() => {
    Stores.Chat.reset()

    const unsubscribe = Stores.EventBus.on(
      'conversation.created',
      event => {
        // Collapse the workspace to a single pane before navigating. Without
        // this, a split left open in the store keeps ConversationPage on its
        // `panes.length >= 2` branch, so the URL→workspace reconcile ("auto
        // while split") REPLACES the focused pane and the old split reappears
        // with the brand-new chat wedged into it — instead of the fresh single
        // conversation that was asked for.
        //
        // Tied to the CREATE, not to this page's mount. Mounting is not a
        // reliable signal of intent: `/` renders this same page (module.tsx),
        // and the router bounces every unmatched path — plus the 403 page's
        // "back to home" link — to `/`. Resetting on mount would therefore
        // destroy an open split, and delete its persisted workspace, when the
        // user merely clicked the logo or followed a stale bookmark. Creating a
        // conversation here is unambiguous, and it is the only moment the
        // hijack can occur.
        //
        // This does NOT touch the in-split "new chat pane" flow: that pane's
        // picker switches to its composer with local state only and never
        // navigates, so this page never mounts and its conversation still
        // adopts into that pane.
        Stores.SplitView.reset()
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
