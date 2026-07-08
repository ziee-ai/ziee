import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useMemo,
  useRef,
} from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import { Flex } from '@/components/ui'
import { Text } from '@/components/ui'
import { Loader2, MessageSquare } from 'lucide-react'
import { ExtensionSlot } from '@/modules/chat/core/extensions'
import { ChatMessage } from '@/modules/chat/components/ChatMessage'
import { Stores } from '@/core/stores'
import {
  captureTopAnchor,
  indexRestoreOffset,
  measureMessageTop,
  restoreDelta,
} from '@/modules/chat/core/utils/scrollAnchor.utils'
import { indexOfMessageId } from '@/modules/chat/core/stores/messageWindow'

/** A captured scroll anchor for reverse-infinite-scroll prepend (ITEM-4). */
export interface MessageAnchor {
  anchorId: string
  /** The anchor row's top relative to the viewport top at capture time. */
  viewportOffset: number
}

/** Imperative API exposed to ConversationPage + the find bar (ITEM-2/4). */
export interface MessageListHandle {
  /** Scroll a loaded message into view. Returns false when the id isn't in the
   *  loaded window (caller falls back to jumpToMessage). */
  scrollToMessageId: (
    id: string,
    align?: 'start' | 'center' | 'end',
  ) => boolean
  /** Scroll to the newest message, settling on the measured bottom. */
  scrollToBottom: () => boolean
  /** Capture the top-most visible message as a prepend anchor. */
  captureAnchor: () => MessageAnchor | null
  /** Re-pin a captured anchor after older messages prepended. */
  restoreAnchor: (anchor: MessageAnchor) => void
}

interface MessageListProps {
  /** The OverlayScrollbars viewport (from ConversationPage), or null before it's
   *  ready. Only meaningful on the virtualized (desktop) path. */
  getScrollElement?: () => HTMLElement | null
  /** Flips true once the scroll viewport exists, forcing the virtualizer to
   *  observe it (mirrors kit/table.tsx's scroll-ready re-render). */
  scrollerReady?: boolean
  /** Virtualize the list. True on desktop (inner OS scroll); FALSE on the mobile
   *  native window-scroll path, where react-virtual can't observe window scroll
   *  without a window-virtualizer — there the (lazy-load-bounded) window renders
   *  plainly with a DOM-based scroll/anchor fallback. Keyed on the stable
   *  `nativeScroll` flag (NOT on scroll-element readiness) so it never flips
   *  mid-session and thrashes the layout. */
  virtualize?: boolean
}

const ESTIMATED_ROW_HEIGHT = 140

/**
 * MessageList — row-virtualized (`@tanstack/react-virtual`) when an inner scroll
 * viewport exists (desktop), so a long loaded window only mounts the visible
 * messages + overscan. On the mobile native-scroll path (no inner viewport /
 * window scroll) it renders the (lazy-load-bounded) window plainly with a
 * DOM-based scroll/anchor fallback — react-virtual can't observe window scroll
 * here without a window-virtualizer. Either way it exposes the same imperative
 * handle so ConversationPage's sentinels / jump / find don't care which path.
 */
export const MessageList = forwardRef<MessageListHandle, MessageListProps>(
  function MessageList(
    {
      getScrollElement = () => null,
      scrollerReady = false,
      virtualize = true,
    }: MessageListProps,
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

    // Live handle to the current array + the plain-path container for the
    // imperative methods.
    const arrRef = useRef(messagesArray)
    arrRef.current = messagesArray
    const plainContainerRef = useRef<HTMLDivElement>(null)

    // A running scroll re-assert (cancellable). A single scrollToIndex can be
    // defeated by concurrent window changes (a bottom-sentinel loadNewer after a
    // jump) + estimate→measured settling; re-assert over a few frames — but
    // cancel on a new call, on unmount, and on a user gesture so it never fights
    // the user or leaks (A2/A4/B3).
    const reassertRef = useRef<(() => void) | null>(null)
    const cancelReassert = () => {
      reassertRef.current?.()
      reassertRef.current = null
    }
    const startReassert = (doScroll: () => void) => {
      cancelReassert()
      const gestureTarget: EventTarget = getScrollElement() ?? window
      let raf = 0
      let n = 0
      const stop = () => {
        if (raf) cancelAnimationFrame(raf)
        gestureTarget.removeEventListener('wheel', stop)
        gestureTarget.removeEventListener('touchmove', stop)
        gestureTarget.removeEventListener('keydown', stop)
        if (reassertRef.current === stop) reassertRef.current = null
      }
      const tick = () => {
        if (n++ >= 3) return stop()
        doScroll()
        raf = requestAnimationFrame(tick)
      }
      raf = requestAnimationFrame(tick)
      // Gesture events fire only on real input — not our own programmatic scroll.
      gestureTarget.addEventListener('wheel', stop, { passive: true })
      gestureTarget.addEventListener('touchmove', stop, { passive: true })
      gestureTarget.addEventListener('keydown', stop)
      reassertRef.current = stop
    }
    useEffect(() => () => cancelReassert(), [])

    useImperativeHandle(
      ref,
      (): MessageListHandle => ({
        scrollToMessageId: (id, align = 'center') => {
          if (virtualize) {
            const doScroll = () => {
              const idx = indexOfMessageId(arrRef.current, id)
              if (idx >= 0) virt.scrollToIndex(idx, { align })
            }
            if (indexOfMessageId(arrRef.current, id) < 0) return false
            doScroll()
            startReassert(doScroll)
            return true
          }
          // Plain path: every loaded row is in the DOM → scroll it into view.
          const el = document.querySelector<HTMLElement>(
            `[data-message-id="${CSS.escape(id)}"]`,
          )
          if (!el) return false
          el.scrollIntoView({
            block: align === 'end' ? 'end' : align === 'start' ? 'start' : 'center',
          })
          return true
        },
        scrollToBottom: () => {
          if (count === 0) return false
          if (virtualize) {
            const doScroll = () => virt.scrollToIndex(count - 1, { align: 'end' })
            doScroll()
            startReassert(doScroll)
          }
          // Plain path: ConversationPage's messagesEndRef handles the jump.
          return true
        },
        captureAnchor: () => {
          if (virtualize) {
            // Compute the top-visible row from the REAL scrollTop + the
            // virtualizer's FULL measurements (all item positions, independent
            // of the rendered range) — NOT virt.scrollOffset or the DOM, both of
            // which lag the actual scrollTop right after a programmatic scroll
            // and yield a stale/wrong anchor → teleport. Restore stays
            // index-based (the row may be virtualized out post-prepend).
            const el = getScrollElement()
            if (!el) return null
            const scrollTop = el.scrollTop
            const item = virt.getVirtualItemForOffset(scrollTop)
            if (!item) return null
            const msg = arrRef.current[item.index]
            if (!msg) return null
            return { anchorId: msg.id, viewportOffset: item.start - scrollTop }
          }
          // Plain path: measure the top-visible row from the DOM (window
          // viewport top = 0).
          const c = plainContainerRef.current
          if (!c) return null
          const a = captureTopAnchor(c, 0)
          return a ? { anchorId: a.anchorId, viewportOffset: a.savedTop } : null
        },
        restoreAnchor: anchor => {
          if (virtualize) {
            const idx = indexOfMessageId(arrRef.current, anchor.anchorId)
            if (idx < 0) return
            const [offsetForIndex] = virt.getOffsetForIndex(idx, 'start') ?? [0]
            virt.scrollToOffset(
              indexRestoreOffset(offsetForIndex ?? 0, anchor.viewportOffset),
            )
            return
          }
          // Plain path: re-pin by the anchor row's new position (window scroll).
          const c = plainContainerRef.current
          if (!c) return
          const newTop = measureMessageTop(c, anchor.anchorId)
          if (newTop == null) return
          const delta = restoreDelta(anchor.viewportOffset, newTop)
          if (delta !== 0) window.scrollBy(0, delta)
        },
      }),
      // Re-create when the virtualizer identity, path, or readiness changes.
      [virt, virtualize, count, scrollerReady],
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
        {/* Top affordance while an OLDER page is being fetched. The aria-live
            region is ALWAYS mounted (only its spinner content toggles) so screen
            readers announce the load. Non-virtualized sibling above the list. */}
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

        {virtualize ? (
          // Virtualized rows. The container spans the full virtual height
          // (getTotalSize) so scroll geometry is preserved; each rendered row is
          // absolutely positioned + self-measured (variable heights). Inter-row
          // spacing lives INSIDE the measured row (py) — a flex gap would be lost
          // under absolute positioning (DEC-6).
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
        ) : (
          // Plain (mobile native-scroll) path: render the whole bounded window.
          <div ref={plainContainerRef} className="flex flex-col gap-1 w-full">
            {messagesArray.map((msg, i) => (
              <ChatMessage
                key={msg.id}
                message={msg}
                isStreaming={
                  isStreaming && i === count - 1 && msg.role === 'assistant'
                }
              />
            ))}
          </div>
        )}

        {/* Streaming indicator (non-virtualized sibling below the list). */}
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
