import { forwardRef, useImperativeHandle, useMemo, useRef } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Flex } from '@/components/ui'
import { Text } from '@/components/ui'
import { Loader2, MessageSquare } from 'lucide-react'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ChatMessage } from '@/modules/chat/components/ChatMessage'
import { Stores } from '@/core/stores'
import { indexRestoreOffset } from '@/modules/chat/core/utils/scrollAnchor.utils'

/** A captured scroll anchor for reverse-infinite-scroll prepend (ITEM-4). */
export interface MessageAnchor {
  anchorId: string
  /** The anchor row's top relative to the viewport top at capture time. */
  viewportOffset: number
}

/** Imperative API exposed to ConversationPage + the find bar (ITEM-2/4). */
export interface MessageListHandle {
  /** Scroll a loaded message into view via the virtualizer. Returns false when
   *  the id isn't in the loaded window (caller falls back to jumpToMessage). */
  scrollToMessageId: (
    id: string,
    align?: 'start' | 'center' | 'end',
  ) => boolean
  /** Capture the top-most visible message as a prepend anchor. */
  captureAnchor: () => MessageAnchor | null
  /** Re-pin a captured anchor after older messages prepended (index-based). */
  restoreAnchor: (anchor: MessageAnchor) => void
}

interface MessageListProps {
  /** The OverlayScrollbars viewport (from ConversationPage), or null until ready.
   *  Optional (defaults to null) so the component can render standalone in the
   *  gallery empty-state surface without a scroll container. */
  getScrollElement?: () => HTMLElement | null
  /** Flips true once the scroll viewport exists, forcing the virtualizer to
   *  observe it (mirrors kit/table.tsx's scroll-ready re-render). */
  scrollerReady?: boolean
}

const ESTIMATED_ROW_HEIGHT = 140

/**
 * MessageList — row-virtualized (`@tanstack/react-virtual`) so a long loaded
 * window only mounts the visible messages + overscan. The virtualizer renders a
 * `getTotalSize()` spacer, so the scroll GEOMETRY is preserved — the lazy-load
 * sentinels + `messagesEndRef` follow in ConversationPage keep working. Only
 * jump/find (off-screen rows) + the prepend anchor go through the imperative
 * handle (`scrollToMessageId` / `captureAnchor` / `restoreAnchor`).
 */
export const MessageList = forwardRef<MessageListHandle, MessageListProps>(
  function MessageList(
    { getScrollElement = () => null, scrollerReady = false }: MessageListProps,
    ref,
  ) {
    const { messages, loading, isStreaming, loadingOlder } = Stores.Chat

    // Ordered window (insertion order = render order).
    const messagesArray = useMemo(() => Array.from(messages.values()), [messages])
    const count = messagesArray.length

    const virt = useVirtualizer({
      count,
      getScrollElement,
      estimateSize: () => ESTIMATED_ROW_HEIGHT,
      overscan: 8,
      // Stable per-message keys so the measurement cache survives prepend /
      // append / window-reset (a message keeps its measured height when its
      // index shifts).
      getItemKey: i => messagesArray[i]?.id ?? i,
    })

    // Keep a live handle to the current array for the imperative methods.
    const arrRef = useRef(messagesArray)
    arrRef.current = messagesArray

    useImperativeHandle(
      ref,
      (): MessageListHandle => ({
        scrollToMessageId: (id, align = 'center') => {
          const idx = arrRef.current.findIndex(m => m.id === id)
          if (idx < 0) return false
          virt.scrollToIndex(idx, { align })
          return true
        },
        captureAnchor: () => {
          const scrollOffset = virt.scrollOffset ?? 0
          for (const vi of virt.getVirtualItems()) {
            // First rendered row whose bottom is below the viewport top.
            if (vi.start + vi.size > scrollOffset) {
              const msg = arrRef.current[vi.index]
              if (!msg) return null
              return { anchorId: msg.id, viewportOffset: vi.start - scrollOffset }
            }
          }
          return null
        },
        restoreAnchor: anchor => {
          const idx = arrRef.current.findIndex(m => m.id === anchor.anchorId)
          if (idx < 0) return
          // Content-space offset of the anchor's (post-prepend) index, then
          // re-pin it at its captured viewport offset. The virtualizer's
          // shouldAdjustScrollPositionOnItemSizeChange (default on) settles the
          // estimate→measured correction of the prepended rows afterwards.
          const [offsetForIndex] = virt.getOffsetForIndex(idx, 'start') ?? [0]
          virt.scrollToOffset(
            indexRestoreOffset(offsetForIndex ?? 0, anchor.viewportOffset),
          )
        },
      }),
      // Re-create when the virtualizer identity or readiness changes.
      [virt, scrollerReady],
    )

    if (!loading && count === 0) {
      return (
        <Flex className={'flex-col gap-1 w-full h-full'} data-testid="chat-messages">
          {/* The "In project" chip and other persistent context markers are NOT
              rendered here — they live in ConversationPage as PINNED chrome above
              the message scroll container so they never scroll out of view. */}
          <div className="flex flex-1 flex-col items-center justify-center text-center py-20">
            <MessageSquare className="text-5xl mb-4" />
            <Text className="text-lg">Start your conversation</Text>
          </div>
        </Flex>
      )
    }

    const virtualItems = virt.getVirtualItems()

    return (
      <Flex className={'flex-col w-full'} data-testid="chat-messages">
        {/* Top affordance while an OLDER page is being fetched (reverse infinite
            scroll). The aria-live region is ALWAYS mounted (only its spinner
            content toggles) so screen readers actually announce the load. A
            fully-loaded-to-top or short conversation just shows an empty region.
            Non-virtualized sibling above the virtual container. */}
        <div
          className="flex w-full justify-center"
          data-testid="chat-loading-older"
          aria-live="polite"
        >
          {loadingOlder && (
            <div className="py-3">
              <Loader2
                className="text-xl animate-spin"
                aria-label="Loading older messages"
              />
            </div>
          )}
        </div>

        {/* Virtualized message rows. The container spans the full virtual height
            (getTotalSize) so scroll geometry is preserved; each rendered row is
            absolutely positioned + self-measured (variable heights). Inter-row
            spacing lives INSIDE the measured row (py) — a flex gap would be lost
            under absolute positioning (DEC-6). */}
        <div
          style={{
            height: virt.getTotalSize(),
            position: 'relative',
            width: '100%',
          }}
        >
          {virtualItems.map(vi => {
            const msg = messagesArray[vi.index]
            if (!msg) return null
            return (
              <div
                key={vi.key}
                data-index={vi.index}
                ref={virt.measureElement}
                style={{
                  position: 'absolute',
                  top: 0,
                  left: 0,
                  width: '100%',
                  transform: `translateY(${vi.start}px)`,
                }}
              >
                <div className="py-0.5">
                  <ChatMessage
                    message={msg}
                    // The streaming message is the last assistant message while a
                    // stream is in flight — never collapse it.
                    isStreaming={
                      isStreaming &&
                      vi.index === count - 1 &&
                      msg.role === 'assistant'
                    }
                  />
                </div>
              </div>
            )
          })}
        </div>

        {/* Streaming indicator (non-virtualized sibling below the container). */}
        {(loading || isStreaming) && (
          <div className={'w-full h-20 mt-3'}>
            <Loader2 className={'text-xl animate-spin'} />
          </div>
        )}

        {/* Extension slot: message list footer */}
        <ExtensionSlot name="message_list_footer" />
      </Flex>
    )
  },
)
