import { useEffect } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import { App, Typography } from 'antd'
import { ChatInput } from '@/modules/chat/components/ChatInput'
import { Stores } from '@/core/stores'

const { Title, Text } = Typography

/**
 * Strict UUID v4-ish check used to validate `?project_id=` query
 * params before latching them into the chat store. Accepts any RFC
 * 4122 lowercase or mixed-case canonical UUID; rejects junk like
 * `garbage` or `'); DROP TABLE`. Closes audit B4: previously a
 * malformed value would only be rejected by the backend at
 * conversation-create time with a confusing 400.
 */
const UUID_RE =
  /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/

export default function NewChatPage() {
  const navigate = useNavigate()
  const { message } = App.useApp()
  const [searchParams] = useSearchParams()
  const rawProjectId = searchParams.get('project_id')

  useEffect(() => {
    Stores.Chat.reset()

    // If the user reached /chat?project_id=<uuid> (e.g. from
    // ProjectDetailPage's "New chat" button or ProjectsNavWidget's
    // hover affordance), latch the project ID into the chat store so
    // the first send creates a conversation INSIDE that project. The
    // store consumes + clears it on the createConversation call.
    if (rawProjectId) {
      if (UUID_RE.test(rawProjectId)) {
        Stores.Chat.setPendingProjectId(rawProjectId)
      } else {
        // Surface the bad URL early instead of failing silently at
        // first send. The user typed/copy-pasted a malformed link.
        message.error(
          'Invalid project link — starting a normal chat instead.',
        )
      }
    }

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
  }, [rawProjectId])

  return (
    <main className="flex flex-col h-full items-center justify-center p-8">
      <div className="w-full max-w-3xl">
        {/* Welcome message */}
        <div className="text-center mb-12">
          <Title level={2}>How can I help you today?</Title>
          <Text type="secondary" className="text-lg">
            Start a new conversation by typing a message below
          </Text>
        </div>

        {/* Chat input */}
        <div className="w-full">
          <ChatInput />
        </div>
      </div>
    </main>
  )
}
