/**
 * chats-page-virtualization ITEM-7 — a long (≈200) conversation list driving the
 * REAL `VirtualizedConversationList` inside a fixed-height scroll box, so the
 * window / scroll / no-jank e2e (TEST-4..6) can measure virtualization on a
 * surface that reproduces the scale-performance case (many variable-height
 * conversation cards). Backend-free: the rows are synthetic `ConversationResponse`
 * objects, so the whole surface renders offline.
 *
 * The correction counter it exercises is exposed on `window.__CHATLIST_METRICS__`
 * by VirtualizedConversationList (DEV-only). Mirrors `MessageListLongDemo.tsx`,
 * which drives the real MessageList the same way.
 */
import { useMemo, useRef, useState, useEffect } from 'react'
import type { ConversationResponse } from '@/api-client/types'
import { Text } from '@/components/ui'
import { ConversationCard } from '@/modules/chat/components/ConversationCard'
import { VirtualizedConversationList } from '@/modules/chat/components/VirtualizedConversationList'

const NOW = new Date('2026-07-08T00:00:00Z')

// Deterministic mixed titles so row heights VARY (1-line vs 2-line wrap) — the
// case that exercises measured-height correction. Kept offline (no fetch).
const SHORT = 'Quick question'
const MEDIUM = 'Debugging the reverse-infinite-scroll anchor on the message list'
const LONG =
  'A deliberately long conversation title that will certainly wrap onto a ' +
  'second rendered line at the content width so the row is measurably taller ' +
  'than a single-line card and line-clamp-2 applies'

function buildConversations(n: number): ConversationResponse[] {
  const out: ConversationResponse[] = []
  for (let i = 0; i < n; i++) {
    const title = i % 5 === 0 ? LONG : i % 2 === 0 ? MEDIUM : SHORT
    const updated = new Date(NOW.getTime() - i * 3_600_000).toISOString()
    out.push({
      id: `g-conv-${String(i).padStart(4, '0')}`,
      title: `${title} (#${i})`,
      message_count: i % 3 === 0 ? 0 : (i % 40) + 1,
      created_at: updated,
      updated_at: updated,
      user_id: 'gallery-user',
    } as ConversationResponse)
  }
  return out
}

export function ConversationListLongDemo({
  count = 200,
  narrow = false,
}: {
  count?: number
  narrow?: boolean
}) {
  const scrollRef = useRef<HTMLDivElement>(null)
  const [ready, setReady] = useState(false)
  const conversations = useMemo(() => buildConversations(count), [count])

  // Flip ready after mount so the virtualizer observes the real scroll box
  // (mirrors the MessageListLongDemo `ready` gate).
  useEffect(() => setReady(true), [])

  return (
    <div
      className="flex w-full flex-col gap-2"
      style={narrow ? { maxWidth: 390 } : undefined}
    >
      <div
        ref={scrollRef}
        data-testid="chat-conversation-list-scroll"
        className="h-[600px] w-full overflow-y-auto border border-border bg-background"
      >
        <div className="mx-auto w-full max-w-4xl">
          <VirtualizedConversationList
            conversations={conversations}
            virtualize
            getScrollElement={() => scrollRef.current}
            scrollerReady={ready}
            renderCard={conversation => (
              <ConversationCard
                conversation={conversation}
                isSelected={false}
                isInSelectionMode={false}
                onSelect={() => {}}
                onDelete={async () => {}}
              />
            )}
            footer={
              <div className="text-center px-3 py-2 flex flex-col items-center gap-2">
                {/* No testid here — `chat-history-pagination-card` is the REAL
                    ConversationList footer's id (must stay globally unique); this
                    demo footer is asserted by its visible text instead. */}
                <Text type="secondary" aria-live="polite" role="status">
                  Showing {conversations.length} of {conversations.length}{' '}
                  conversations
                </Text>
              </div>
            }
          />
        </div>
      </div>
    </div>
  )
}
