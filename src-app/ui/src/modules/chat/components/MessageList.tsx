import {
  forwardRef,
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual'
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
import { inPlaceAnchorSignal } from '@/modules/chat/core/utils/useInPlaceAnchor'
import {
  buildInitialMeasurementsCache,
  recordMeasurements,
  widthBucket,
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
/** Stable empty seed (identity kept so an empty window doesn't churn options). */
const EMPTY_SEED: VirtualItem[] = []

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

    // The rendered content width drives the height estimate + the measured-cache
    // width bucket. It is tracked in a REF (updated by a ResizeObserver below),
    // NOT read from the DOM on demand: `estimateSize` runs inside the render that
    // the virtualizer triggers right after writing layout (a measurement +
    // scroll adjustment), so a `clientWidth` read there would force a synchronous
    // reflow on every measurement during scroll. The ref read is free. A coarse
    // width BUCKET is mirrored into state so the seed memo below rebuilds when the
    // viewport actually changes size class (FIX_ROUND-1: reflow + resize-seed).
    const widthRef = useRef(MAX_CONTENT_WIDTH - CONTENT_GUTTER)
    const [widthBucketState, setWidthBucketState] = useState(() =>
      widthBucket(widthRef.current),
    )
    const measureWidth = () => {
      const vw = getScrollElement()?.clientWidth
      if (!vw || vw <= 0) return
      const w = Math.min(vw, MAX_CONTENT_WIDTH) - CONTENT_GUTTER
      if (w <= 0) return
      widthRef.current = w
      const b = widthBucket(w)
      setWidthBucketState(prev => (prev === b ? prev : b))
    }
    // useLayoutEffect so `widthRef` is set BEFORE the first paint — on a warm
    // reopen the messages load asynchronously (count 0 → N a tick later), so by
    // the time the seed is consumed the width (and its bucket) is already the
    // real one, not the fallback (FIX_ROUND-1: seed bucket mismatch).
    useLayoutEffect(() => {
      const el = getScrollElement()
      if (!el) return
      measureWidth()
      const ro = new ResizeObserver(measureWidth)
      ro.observe(el)
      return () => ro.disconnect()
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [scrollerReady])

    // Seed the virtualizer with any REAL measured heights persisted from a prior
    // mount of this (or any) conversation at this width bucket, so re-opening a
    // long conversation starts rows at their true height (near-zero first-scroll
    // correction) (ITEM-2, DEC-2).
    //
    // `@tanstack/virtual-core` consumes `initialMeasurementsCache` EXACTLY ONCE —
    // at the first render where the window is non-empty (its internal
    // measurementsCache is still empty). The store clears messages on every
    // conversation switch, so a real mount starts at count 0 and the count 0→N
    // transition happens AFTER the layout effect corrected `widthRef` — so the
    // one build lands at the right width bucket. We FREEZE it (seedRef) after that
    // first non-empty build: streaming replaces the messages Map on every token,
    // churning `messagesArray` identity, but rebuilding a seed the library has
    // already consumed is wasted O(window) work + an impure LRU mutation during
    // render (FIX_ROUND-2). A mid-session width change can't re-seed (the library
    // won't re-read it) — nor does it need to: the visible rows simply re-measure
    // at the new width via the row ResizeObservers.
    const seedRef = useRef<VirtualItem[] | null>(null)
    const initialMeasurementsCache = useMemo(
      () => {
        // Window reset (conversation switch clears messages to an empty Map, then
        // virtual-core empties its measurementsCache too) → drop the freeze so the
        // NEXT non-empty build seeds the NEW conversation's ids, not the stale
        // previous one. Streaming never reaches count 0, so it never resets here —
        // the seed stays frozen through a stream (FIX_ROUND-2).
        if (messagesArray.length === 0) {
          seedRef.current = null
          return EMPTY_SEED
        }
        if (seedRef.current) return seedRef.current
        const seed = buildInitialMeasurementsCache(
          messagesArray.map(m => m.id),
          widthRef.current,
        )
        seedRef.current = seed
        return seed
      },
      // widthBucketState is a dep only so the pre-consume (count 0) width
      // correction is reflected in the single build; after the freeze the body
      // short-circuits. getScrollElement is a fresh closure each render, not a dep.
      // eslint-disable-next-line react-hooks/exhaustive-deps
      [messagesArray, widthBucketState],
    )

    // Debounced measured-height write-back. The virtualizer's onChange fires with
    // sync=false on EVERY row measurement (not only at scroll-end), so folding
    // the whole itemSizeCache there per event would be O(n²) over a scroll-
    // through. Instead, coalesce into ONE trailing flush ~after measurements
    // settle (FIX_ROUND-1: O(n²) write-back).
    // DEV-only virtualizer-correction metrics (ITEM-1). Kept in a ref so counting
    // never triggers a render; surfaced on `window.__MSGLIST_METRICS__` by the
    // effect below for the scroll-stability e2e.
    const metricsRef = useRef({ corrections: 0 })

    const flushTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined)
    const scheduleFlush = () => {
      if (flushTimer.current) clearTimeout(flushTimer.current)
      flushTimer.current = setTimeout(() => {
        flushTimer.current = undefined
        recordMeasurements(virt.itemSizeCache, widthRef.current)
      }, 400)
    }

    const virt = useVirtualizer({
      count,
      getScrollElement,
      // Content-aware first-pass estimate (per-message: text length + table /
      // image / code / tool add-ons), memoized per (message, width bucket), so
      // the estimate→measured correction — and the scrollbar-thumb jump it caused
      // — shrinks toward zero. Reads the width REF (no reflow) (ITEM-1, DEC-1).
      estimateSize: i => estimateMessageHeight(messagesArray[i], widthRef.current),
      // Overscan stays at 8 (the pre-virtualization-fix value). ITEM-5's proposed
      // drop to 4 measurably regressed the reverse-infinite-scroll ANCHOR: fewer
      // off-screen rows above the viewport get measured, so the prepend
      // anchor-restore leans on the (coarser) estimate and the view drifts ~120px
      // (the `lazy-load-messages` anchor invariant broke). Anchor precision beats
      // the marginal off-screen-mount saving — especially since ChatMessage is
      // memoized (extra overscan rows don't re-render on scroll) (ITEM-5, DEC-5;
      // FIX_ROUND-4 drift).
      overscan: 8,
      initialMeasurementsCache,
      // Persist real measured heights across mounts. sync=true is scroll,
      // sync=false is a measurement/layout change — coalesce those into one
      // trailing flush (never per scroll frame; O(n) once per settle) (ITEM-2).
      onChange: (_instance, sync) => {
        if (!sync) {
          scheduleFlush()
          // ITEM-1 instrumentation: a non-sync onChange is a size/layout
          // RECORRECTION (an item re-measured → total-size recompute), which is
          // exactly the scrollbar-jump signal. Count them so the e2e can assert
          // they settle to ~0 after a scroll pause. DEV-only (tree-shaken out).
          if (import.meta.env.DEV) metricsRef.current.corrections++
        }
      },
      // Stable per-message keys so the measurement cache survives prepend /
      // append / window-reset (a message keeps its measured height when its
      // index shifts).
      getItemKey: i => messagesArray[i]?.id ?? i,
    })

    // ITEM-7: when the user intentionally changes a row's height in place
    // (show-more / inline-file resize), suppress the virtualizer's OWN
    // above-fold scroll compensation for THAT row (its key is parked in
    // inPlaceAnchorSignal) so the row grows downward from its current top instead
    // of the viewport teleporting when the row straddles the top fold;
    // useInPlaceAnchor then pins any residual drift. For every other row this
    // replicates virtual-core's default predicate (adjust above-viewport rows
    // whose size changes, except re-measures while scrolling backward).
    // `@tanstack/react-virtual` (resolved virtual-core 3.17.3) READS
    // `shouldAdjustScrollPositionOnItemSizeChange` as an instance property
    // (resizeItem, index.js:869) but does not accept it as a typed option, so it
    // is assigned imperatively here (idempotent per render; virtual-core never
    // overwrites it). For every NON-parked row this must FAITHFULLY replicate the
    // library's default predicate — including the `+ scrollAdjustments` term and
    // `getScrollOffset()` (NOT the raw `scrollOffset`), which accumulate during a
    // measurement burst — or above-fold rows the library would adjust get
    // skipped, reintroducing estimate-correction / prepend-anchor drift.
    ;(
      virt as unknown as {
        shouldAdjustScrollPositionOnItemSizeChange?: (
          item: VirtualItem,
          delta: number,
          instance: typeof virt,
        ) => boolean
      }
    ).shouldAdjustScrollPositionOnItemSizeChange = (item, _delta, instance) => {
      if (inPlaceAnchorSignal.key != null && item.key === inPlaceAnchorSignal.key) {
        return false
      }
      const inst = instance as unknown as {
        getScrollOffset: () => number
        scrollAdjustments: number
        itemSizeCache: { has: (k: VirtualItem['key']) => boolean }
        scrollDirection: 'forward' | 'backward' | null
      }
      const off = inst.getScrollOffset() + inst.scrollAdjustments
      return (
        item.start < off &&
        (!inst.itemSizeCache.has(item.key) || inst.scrollDirection !== 'backward')
      )
    }

    // Flush measured heights on unmount (conversation close / navigate away) so
    // the next open seeds from them. Uses widthRef (last-known-good width) — the
    // scroll DOM may already be detached in cleanup, so a clientWidth read would
    // be 0 and record under the wrong bucket (FIX_ROUND-1: unmount bucket).
    useEffect(() => {
      return () => {
        if (flushTimer.current) clearTimeout(flushTimer.current)
        recordMeasurements(virt.itemSizeCache, widthRef.current)
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [])

    // DEV-only: surface the correction counter + live total size so the
    // scroll-stability e2e (TEST-6) can reset, scroll, and assert the count
    // settles to ~0 after each pause. Compiled out of production builds.
    useEffect(() => {
      if (!import.meta.env.DEV) return
      const w = window as unknown as {
        __MSGLIST_METRICS__?: {
          corrections: number
          reset: () => void
          totalSize: () => number
        }
      }
      w.__MSGLIST_METRICS__ = {
        get corrections() {
          return metricsRef.current.corrections
        },
        reset: () => {
          metricsRef.current.corrections = 0
        },
        totalSize: () => virt.getTotalSize(),
      }
      return () => {
        delete w.__MSGLIST_METRICS__
      }
      // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [virt])

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
