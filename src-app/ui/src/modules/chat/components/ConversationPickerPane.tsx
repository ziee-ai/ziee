import { useEffect, useMemo, useState } from 'react'
import { ArrowLeft, MessageSquarePlus, Search, X } from 'lucide-react'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import { Button, Empty, Input, Title, Tooltip } from '@/components/ui'
import { Stores } from '@/core'
import { ChatInput } from '@/modules/chat/components/ChatInput'

dayjs.extend(relativeTime)

/**
 * The empty-pane conversation picker (ITEM-27 / FB-3). A split pane that holds no
 * conversation (`conversationId: null`) is the second slot of a split waiting to
 * be filled — v1 hard-wired it to a bare "New chat" composer, so a split could
 * only ever be `[current | new-chat]` and NEVER placed two EXISTING conversations
 * side by side. This renders a searchable list of the user's conversations so the
 * slot can hold an existing one (`setPaneConversation`), with "Start a new chat"
 * still available (→ the existing new-chat adopt path).
 *
 * Rendered inside the pane's `ChatPaneProvider` subtree, so the new-chat composer
 * (`ChatInput`) + adoption effect (in `ConversationPane`) work per-pane unchanged.
 */
export function ConversationPickerPane({ paneId }: { paneId: string }) {
  const [mode, setMode] = useState<'pick' | 'new'>('pick')
  const [query, setQuery] = useState('')
  const { conversations, isInitialized } = Stores.ChatHistory

  useEffect(() => {
    if (!isInitialized) Stores.ChatHistory.loadConversations()
  }, [isInitialized])

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase()
    // A conversation already open in another pane can't be opened again (one per
    // workspace) — hide those already targeted by a pane so the list only offers
    // openable conversations.
    const openIds = new Set(
      Stores.SplitView.$.panes
        .map((p) => p.conversationId)
        .filter((id): id is string => id != null),
    )
    return conversations.filter((c) => {
      if (openIds.has(c.id)) return false
      if (!q) return true
      return (c.title || 'Untitled Conversation').toLowerCase().includes(q)
    })
  }, [conversations, query])

  const header = (
    <div
      className="flex h-11 shrink-0 items-center justify-between gap-2 border-b px-3"
      data-testid="chat-pane-header"
    >
      <span className="min-w-0 truncate text-sm text-muted-foreground">
        {mode === 'new' ? 'New chat' : 'Open a conversation'}
      </span>
      <Tooltip content="Close pane">
        <Button
          data-testid="chat-pane-close"
          variant="ghost"
          size="icon"
          icon={<X />}
          aria-label="Close pane"
          onClick={() => Stores.SplitView.closePane(paneId)}
        />
      </Tooltip>
    </div>
  )

  // New-chat mode: the greeting + composer (adopted into this pane's slot on
  // first send by `ConversationPane`). A back affordance returns to the picker.
  if (mode === 'new') {
    return (
      <div className="flex h-full flex-col" data-testid="conversation-picker-pane">
        {header}
        <div className="flex flex-1 min-h-0 flex-col items-center justify-center p-4">
          <div className="w-full max-w-3xl">
            <div className="mb-4 flex justify-start">
              <Button
                data-testid="pane-picker-back"
                variant="ghost"
                icon={<ArrowLeft />}
                onClick={() => setMode('pick')}
              >
                Browse conversations
              </Button>
            </div>
            <div className="mb-8 text-center">
              <Title level={2} data-testid="pane-new-chat-greeting">
                How can I help you today?
              </Title>
            </div>
            <ChatInput />
          </div>
        </div>
      </div>
    )
  }

  return (
    <div className="flex h-full flex-col" data-testid="conversation-picker-pane">
      {header}
      <div className="flex flex-col gap-2 p-3 min-h-0 flex-1">
        <Button
          data-testid="pane-start-new-chat"
          variant="outline"
          className="w-full justify-start"
          icon={<MessageSquarePlus />}
          onClick={() => setMode('new')}
        >
          Start a new chat
        </Button>
        <Input
          data-testid="conversation-picker-search"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
          placeholder="Search conversations..."
          prefix={<Search className="size-4 text-muted-foreground" />}
          aria-label="Search conversations"
        />
        <div className="min-h-0 flex-1 overflow-y-auto" data-testid="conversation-picker-list">
          {filtered.length === 0 ? (
            <Empty
              data-testid="conversation-picker-empty"
              className="py-8"
              description={
                conversations.length === 0
                  ? 'No conversations yet'
                  : 'No conversations match your search'
              }
            />
          ) : (
            <ul className="flex flex-col">
              {filtered.map((c) => (
                <li key={c.id}>
                  <Button
                    variant="ghost"
                    data-testid={`conversation-picker-item-${c.id}`}
                    className="h-auto w-full justify-between gap-2 px-2 py-2 font-normal"
                    onClick={() =>
                      Stores.SplitView.setPaneConversation(paneId, c.id)
                    }
                  >
                    <span className="min-w-0 flex-1 truncate text-start text-sm">
                      {c.title || 'Untitled Conversation'}
                    </span>
                    <span className="shrink-0 text-xs text-muted-foreground">
                      {dayjs(c.updated_at).fromNow()}
                    </span>
                  </Button>
                </li>
              ))}
            </ul>
          )}
        </div>
      </div>
    </div>
  )
}
