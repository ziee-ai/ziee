import {
  type ReactNode,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from 'react'
import { useVirtualizer, type VirtualItem } from '@tanstack/react-virtual'
import type { ConversationResponse } from '@/api-client/types'
import { estimateConversationHeight } from '@/modules/chat/core/utils/estimateConversationHeight'
import {
  makeChatListMetrics,
} from '@/modules/chat/core/utils/chatListMetrics'
import {
  buildInitialMeasurementsCache,
  recordMeasurements,
  widthBucket,
} from '@/modules/chat/core/utils/measuredHeightCache'

/**
 * VirtualizedConversationList (chats-page-virtualization ITEM-3/4/6)
 *
 * Row-virtualizes the `/chats` conversation list — a direct analog of
 * `MessageList.tsx`'s virtualized branch. Only the visible window (+ overscan) of
 * `ConversationCard` rows is mounted; scroll geometry is preserved by a full-
 * height (`getTotalSize`) container with absolutely-positioned, self-measured
 * rows (variable title-wrap heights). It reuses the SAME id-generic measured-
 * height cache + width-bucket helpers as MessageList (DEC-2) and the same
 * content-aware `estimateSize` pattern, so the estimate→measured correction (the
 * scrollbar-thumb "jank") stays near zero and survives a re-open of the page.
 *
 * Desktop (inner OverlayScrollbars viewport) virtualizes. On the mobile native-
 * window-scroll path (`virtualize=false`, no inner viewport) it renders the
 * paging-bounded set plainly — react-virtual can't observe window scroll without
 * a window-virtualizer, exactly as MessageList does.
 *
 * It is a PURE rendering layer: it owns no data and no store. The caller passes
 * the ordered `conversations`, a `renderCard` for each row, and the resolved
 * scroll element. The pagination footer is a non-virtualized sibling the caller
 * supplies via `footer`.
 */

/** App content column: max-w-4xl (896px) minus the px-3 row gutters (24px). */
const MAX_CONTENT_WIDTH = 896
const CONTENT_GUTTER = 24
const OVERSCAN = 8
/**
 * Vertical padding the Row wrapper (`py-1.5` = 6px top + 6px bottom) adds AROUND
 * the card. `measureElement` measures the wrapper, so the size estimate must
 * include it or every row corrects by ~12px on first measure (a systematic
 * estimate→measured jump). The pure card estimator stays card-only; the wrapper
 * padding is added HERE, at the single measured-element boundary.
 */
const ROW_VERTICAL_PADDING = 12
/** Stable empty seed identity (so an empty window doesn't churn the option). */
const EMPTY_SEED: VirtualItem[] = []


interface VirtualizedConversationListProps {
  /** Ordered conversation rows (render order = array order). */
  conversations: ConversationResponse[]
  /** Render one card for a conversation (caller owns selection/delete wiring). */
  renderCard: (conversation: ConversationResponse) => ReactNode
  /** The inner scroll viewport, or null before it's ready / on the mobile path. */
  getScrollElement: () => HTMLElement | null
  /** Flips true once the OverlayScrollbars viewport exists (re-observe trigger). */
  scrollerReady: boolean
  /** Virtualize (desktop inner scroll) vs plain render (mobile window scroll). */
  virtualize: boolean
  /** Non-virtualized footer sibling (the "Showing N of M" + Load-More block). */
  footer?: ReactNode
}

/**
 * Row layout wrapper — the horizontal gutter (`px-3`) + inter-row spacing
 * (`py-1.5`, internalized because a flex gap is lost under absolute positioning,
 * DEC-5). This element is what `measureElement` measures (hence
 * `ROW_VERTICAL_PADDING` in the size estimate). Scroll-render cost is avoided by
 * the CALLER passing a MEMOIZED card whose props are stable (DEC-9) — memoizing
 * this wrapper is futile since `children` is a fresh element each render.
 */
function Row({ children }: { children: ReactNode }) {
  return <div className="px-3 py-1.5">{children}</div>
}

export function VirtualizedConversationList({
  conversations,
  renderCard,
  getScrollElement,
  scrollerReady,
  virtualize,
  footer,
}: VirtualizedConversationListProps) {
  const count = conversations.length

  // Rendered content width drives the estimate + the measured-cache width bucket.
  // Tracked in a REF (updated by a ResizeObserver) so `estimateSize` never forces
  // a synchronous reflow during a measurement burst; a coarse BUCKET is mirrored
  // into state so the seed memo rebuilds when the viewport changes size class.
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
  useLayoutEffect(() => {
    const el = getScrollElement()
    if (!el) return
    measureWidth()
    const ro = new ResizeObserver(measureWidth)
    ro.observe(el)
    return () => ro.disconnect()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scrollerReady])

  // Seed with any REAL measured heights persisted from a prior mount of these
  // conversations at this width bucket, so re-opening /chats starts rows at their
  // true height (near-zero first-scroll correction). virtual-core consumes the
  // seed EXACTLY ONCE (first non-empty render); FREEZE it after that so a store
  // update that churns the array identity doesn't rebuild a consumed seed. Reset
  // on an empty list so the next non-empty build seeds the NEW result set (e.g.
  // after a search filter clears) — mirrors MessageList.
  const seedRef = useRef<VirtualItem[] | null>(null)
  const initialMeasurementsCache = useMemo(
    () => {
      if (count === 0) {
        seedRef.current = null
        return EMPTY_SEED
      }
      if (seedRef.current) return seedRef.current
      const seed = buildInitialMeasurementsCache(
        conversations.map(c => c.id),
        widthRef.current,
      )
      seedRef.current = seed
      return seed
    },
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [conversations, widthBucketState],
  )

  // DEV-only correction metrics (kept in a ref so counting never renders).
  const metricsRef = useRef({ corrections: 0 })

  // Coalesced measured-height write-back: onChange fires with sync=false on every
  // row measurement, so fold the whole itemSizeCache in ONE trailing flush after
  // measurements settle (never O(n) per event) — mirrors MessageList.
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
    estimateSize: i =>
      estimateConversationHeight(conversations[i], widthRef.current) +
      ROW_VERTICAL_PADDING,
    overscan: OVERSCAN,
    initialMeasurementsCache,
    onChange: (_instance, sync) => {
      if (!sync) {
        scheduleFlush()
        // A non-sync onChange is a size RECORRECTION (a row re-measured → total-
        // size recompute) — the scrollbar-jump signal. Count it so the e2e can
        // assert it settles to ~0 after a scroll pause. DEV-only (tree-shaken).
        if (import.meta.env.DEV) metricsRef.current.corrections++
      }
    },
    // Stable per-conversation keys so the measurement cache survives append /
    // prepend / removal (a row keeps its measured height when its index shifts).
    getItemKey: i => conversations[i]?.id ?? i,
  })

  // Flush measured heights on unmount (navigate away) so the next open seeds from
  // them. Uses widthRef (last-known-good) — the scroll DOM may be detached in
  // cleanup, so a clientWidth read would be 0 and record under the wrong bucket.
  useEffect(() => {
    return () => {
      if (flushTimer.current) clearTimeout(flushTimer.current)
      recordMeasurements(virt.itemSizeCache, widthRef.current)
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // DEV-only: surface the correction counter + live total size so the no-jank
  // e2e (TEST-6) can reset, scroll, and assert the count settles. Compiled out of
  // production builds.
  useEffect(() => {
    if (!import.meta.env.DEV) return
    window.__CHATLIST_METRICS__ = makeChatListMetrics(metricsRef.current, () =>
      virt.getTotalSize(),
    )
    return () => {
      delete window.__CHATLIST_METRICS__
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [virt])

  if (!virtualize) {
    // Plain (mobile native-scroll) path: render the whole paging-bounded set.
    return (
      <div
        className="flex flex-col gap-3 w-full flex-1"
        data-testid="chat-conversation-list-rows"
      >
        {conversations.map(conversation => (
          <div key={conversation.id} className="px-3">
            {renderCard(conversation)}
          </div>
        ))}
        {footer}
      </div>
    )
  }

  const virtualItems = virt.getVirtualItems()

  return (
    <div className="w-full flex flex-col flex-1">
      {/* Virtualized rows: the container spans the full virtual height so scroll
          geometry is preserved; each row is absolutely positioned + self-measured
          (variable title-wrap heights). Inter-row spacing lives INSIDE the
          measured row (py) — a flex gap is lost under absolute positioning
          (DEC-5). */}
      <div
        data-testid="chat-conversation-list-rows"
        style={{
          height: virt.getTotalSize(),
          position: 'relative',
          width: '100%',
        }}
      >
        {virtualItems.map(vi => {
          const conversation = conversations[vi.index]
          if (!conversation) return null
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
              <Row>{renderCard(conversation)}</Row>
            </div>
          )
        })}
      </div>
      {footer}
    </div>
  )
}
