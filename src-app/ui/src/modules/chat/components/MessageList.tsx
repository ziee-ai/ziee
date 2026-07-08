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
  anchorRestoreNeeded,
  captureTopAnchor,
  indexRestoreOffset,
  measureMessageTop,
  restoreDelta,
} from '@/modules/chat/core/utils/scrollAnchor.utils'
import { indexOfMessageId } from '@/modules/chat/core/stores/messageWindow'
import { estimateMessageHeight } from '@/modules/chat/core/utils/estimateMessageHeight'
import {
  buildInitialMeasurementsCache,
  recordMeasurements,
} from '@/modules/chat/core/utils/measuredHeightCache'

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

/** App content column: max-w-4xl (896px) minus the px-4 gutters. */
const MAX_CONTENT_WIDTH = 896
const CONTENT_GUTTER = 32

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

    // The rendered content width (drives the height estimate + the width bucket
    // of the measured-height cache). Read live from the scroll viewport, capped
    // at the app's max-w-4xl column; falls back to the column width before the
    // scroller mounts. Cheap (no layout thrash — clientWidth is already laid
    // out). (message-scroll-perf ITEM-1/ITEM-2, DEC-1/DEC-2.)
    const contentWidth = () => {
      const vw = getScrollElement()?.clientWidth ?? MAX_CONTENT_WIDTH
      return Math.min(vw, MAX_CONTENT_WIDTH) - CONTENT_GUTTER
    }

    // Seed the virtualizer with any REAL measured heights persisted from a prior
    // mount of this (or any) conversation at this width bucket, so re-opening a
    // long conversation starts rows at their true height (near-zero first-scroll
    // correction). Rebuilds only when the message window changes — never on
    // scroll (ITEM-2, DEC-2).
    const initialMeasurementsCache = useMemo(
      () => buildInitialMeasurementsCache(messagesArray.map(m => m.id), contentWidth()),
      // contentWidth is read at compute time; getScrollElement is intentionally
      // not a dep (a fresh closure each ConversationPage render).
      // eslint-disable-next-line react-hooks/exhaustive-deps
      [messagesArray],
    )

    const virt = useVirtualizer({
      count,
      getScrollElement,
      // Content-aware first-pass estimate (per-message: text length + table /
      // image / code / tool add-ons) so the estimate→measured correction — and
      // the scrollbar-thumb jump it caused — shrinks toward zero (ITEM-1, DEC-1).
      estimateSize: i => estimateMessageHeight(messagesArray[i], contentWidth()),
      // Fewer heavy off-screen tables/images mounted per frame; pop-in still
      // acceptable at normal scroll speed (ITEM-5, DEC-5).
      overscan: 4,
      initialMeasurementsCache,
      // Persist real measured heights across mounts. `sync` is true on scroll
      // and false on a measurement/layout change, so this fires only on the
      // (bounded) measurement events — never per scroll frame — and reads the
      // virtualizer's OWN size map (no second observer) (ITEM-2, DEC-2).
      onChange: (instance, sync) => {
        if (!sync) recordMeasurements(instance.itemSizeCache, contentWidth())
      },
      // Stable per-message keys so the measurement cache survives prepend /
      // append / window-reset (a message keeps its measured height when its
      // index shifts).
      getItemKey: i => messagesArray[i]?.id ?? i,
    })

    // Flush measured heights on unmount (conversation close / navigate away) so
    // the next open seeds from them even if no measurement event fired late in
    // the session (ITEM-2).
    useEffect(() => {
      return () => recordMeasurements(virt.itemSizeCache, contentWidth())
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [])

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
            const target = indexRestoreOffset(
              offsetForIndex ?? 0,
              anchor.viewportOffset,
            )
            // Skip the explicit restore when the virtualizer's own
            // above-viewport size-change adjustment already pinned the anchor
            // (within tolerance) — avoids a redundant scroll that double-adjusts
            // into a visible jump (ITEM-6, DEC-6). The before-paint restore is
            // preserved for the common case where it hasn't yet.
            const el = getScrollElement()
            if (el && !anchorRestoreNeeded(el.scrollTop, target)) return
            virt.scrollToOffset(target)
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
