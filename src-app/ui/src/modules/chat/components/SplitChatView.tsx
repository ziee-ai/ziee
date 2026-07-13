import { Fragment, useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'
import { Stores } from '@/core'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { ChatPaneProvider } from '@/modules/chat/core/pane/ChatPaneContext'
import { ConversationPane } from '@/modules/chat/pages/ConversationPage'
import { PaneTabStrip } from '@/modules/chat/components/PaneTabStrip'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'

/**
 * In-window split chat (ITEM-8). Renders the `SplitView` store's ordered panes
 * side-by-side, each an independent `ConversationPane` bound to its own
 * `ChatPaneStore` via `ChatPaneProvider`. A vertical drag divider between panes
 * resizes the left pane (`dividerWidths`); the focused pane carries an inset ring
 * (no dimming of the others). Pointer-down anywhere in a pane focuses it, so
 * handler-time `Stores.Chat` actions route to the pane the user is acting on.
 *
 * Only mounted for ≥2 panes (the brancher in `ConversationPage`); single-pane
 * stays on the plain `ConversationPane` path.
 */
export function SplitChatView() {
  // Split is a desktop layout — force the inner-scroll shell for the whole view.
  useNativeScroll(false)
  const { panes, focusedPaneId, dividerWidths } = Stores.SplitView
  // At/below `md` (≤768px) there isn't room to tile columns → tab mode (ITEM-30):
  // one visible pane + a tab strip. All panes stay MOUNTED (only the focused one is
  // shown) so a background pane keeps streaming. `useWindowMinSize().md` is TRUE
  // when the viewport is AT MOST 768px (mobile/tablet), so tab mode is `if (md)` —
  // desktop (md === false) tiles columns.
  const { md } = useWindowMinSize()

  // ONE tree for both modes (not two branches): crossing the `md` breakpoint must
  // NOT change a pane's element TYPE or its key, or React would unmount + remount
  // every `ChatPaneProvider` (recreating the per-pane store → tearing down live
  // streams). Each pane is always `<Fragment key=paneId><div key="pane">`; the
  // divider (columns-only) and the tab strip (tabs-only) toggle around it, and the
  // KEYED children keep the pane div reconciled by key regardless (DRIFT-2.14).
  return (
    <div
      className={cn(
        'flex h-full min-h-0 w-full overflow-hidden',
        md && 'flex-col',
      )}
      data-testid="split-chat-view"
      data-split-mode={md ? 'tabs' : 'columns'}
    >
      {md && <PaneTabStrip />}
      {panes.map((p, i) => (
        <Fragment key={p.paneId}>
          {!md && i > 0 && <SplitDivider key="divider" leftPaneIndex={i - 1} />}
          {/* biome-ignore lint/a11y/useKeyWithClickEvents: focus-on-interact is
              pointer-scoped; keyboard focus already lands via the pane's own
              focusable controls, and pointerDownCapture never steals it. */}
          <div
            key="pane"
            data-testid={`chat-pane-${i}`}
            role={md ? 'tabpanel' : undefined}
            className={cn(
              'relative min-h-0 flex-col overflow-hidden',
              md
                ? p.paneId === focusedPaneId
                  ? 'flex flex-1'
                  : 'hidden'
                : cn(
                    'flex min-w-0 transition-opacity duration-200',
                    // Subtle focus indicator (DEC-28 amended): NO ring on the
                    // focused pane. Instead DIM the non-focused panes so the active
                    // one reads at full strength. Pointer events are unaffected
                    // (clicking a dimmed pane still focuses it via the
                    // pointer-down-capture below). Dropping the ring also removes
                    // the FB-18 z-index competition with the sidebar toggle, so no
                    // `z-[5]` lift is needed anymore.
                    focusedPaneId === p.paneId ? 'opacity-100' : 'opacity-45',
                  ),
            )}
            style={
              md
                ? undefined
                : { flex: dividerWidths[i] ? `0 0 ${dividerWidths[i]}px` : '1 1 0%' }
            }
            onPointerDownCapture={() => Stores.SplitView.focusPane(p.paneId)}
          >
            <ChatPaneProvider paneId={p.paneId} conversationId={p.conversationId}>
              <ConversationPane />
            </ChatPaneProvider>
          </div>
        </Fragment>
      ))}
    </div>
  )
}

/**
 * Vertical drag divider between two panes. Dragging sets the LEFT pane's width
 * (`dividerWidths[leftPaneIndex]`, clamped in the store to MIN/MAX). Reads the
 * left pane's current rendered width on pointer-down so a flex-sized (unpinned)
 * pane resizes smoothly from where it actually is.
 */
function SplitDivider({ leftPaneIndex }: { leftPaneIndex: number }) {
  const ref = useRef<HTMLDivElement>(null)
  const drag = useRef<{ x: number; w: number; last: number } | null>(null)
  // The seam is a pure resize handle now (ITEM-70): dropping a conversation to
  // insert a pane is handled by the per-pane edge-directional zones (a pane's
  // left/right third), so the divider no longer owns a conversation drop-zone.
  // Live index in a ref so the stable window handlers always resize the CURRENT
  // divider even after a pane reorder changed this divider's leftPaneIndex.
  const idxRef = useRef(leftPaneIndex)
  idxRef.current = leftPaneIndex
  const width = Stores.SplitView.dividerWidths[leftPaneIndex]

  // Stable (created-once) window handlers so add/removeEventListener always
  // match AND unmount cleanup can remove them — fixes the listener leak when the
  // divider unmounts mid-drag (a pane is closed/reordered during a resize).
  const handlers = useRef({
    move: (e: PointerEvent) => {
      const d = drag.current
      if (!d) return
      // IMPERATIVE resize (perf): a drag writes the LEFT pane's flex-basis STRAIGHT
      // to the DOM — it does NOT touch the SplitView store per frame. Otherwise
      // every pointermove sets `dividerWidths`, which `SplitChatView` reads
      // reactively, re-rendering the WHOLE tree (both full chat panes, ~95ms/frame
      // → the reported lag). Clamped identically to the store; committed ONCE on
      // pointer-up so the store + persistence hold the final width.
      const w = Math.max(
        SPLIT_LIMITS.MIN_PANE_WIDTH,
        Math.min(
          SPLIT_LIMITS.MAX_PANE_WIDTH,
          Math.round(d.w + (e.clientX - d.x)),
        ),
      )
      d.last = w
      const leftEl = ref.current?.previousElementSibling as HTMLElement | null
      if (leftEl) leftEl.style.flex = `0 0 ${w}px`
    },
    up: () => {
      const d = drag.current
      // Commit the final width once — React then reconciles the pane's inline
      // style to the same clamped value (no visual jump).
      if (d) Stores.SplitView.setDividerWidth(idxRef.current, d.last)
      drag.current = null
      window.removeEventListener('pointermove', handlers.current.move)
      window.removeEventListener('pointerup', handlers.current.up)
    },
  })

  useEffect(() => {
    const h = handlers.current
    return () => {
      window.removeEventListener('pointermove', h.move)
      window.removeEventListener('pointerup', h.up)
    }
  }, [])

  const currentLeftWidth = (): number => {
    const leftEl = ref.current?.previousElementSibling as HTMLElement | null
    return (
      leftEl?.getBoundingClientRect().width ??
      width ??
      SPLIT_LIMITS.MIN_PANE_WIDTH
    )
  }

  const onDown = (e: React.PointerEvent) => {
    const w = currentLeftWidth()
    drag.current = { x: e.clientX, w, last: w }
    window.addEventListener('pointermove', handlers.current.move)
    window.addEventListener('pointerup', handlers.current.up)
  }

  // Keyboard resize (WCAG 2.1.1): arrows nudge the left pane's width.
  const onKeyDown = (e: React.KeyboardEvent) => {
    const STEP = 24
    if (e.key === 'ArrowLeft') {
      e.preventDefault()
      Stores.SplitView.setDividerWidth(leftPaneIndex, currentLeftWidth() - STEP)
    } else if (e.key === 'ArrowRight') {
      e.preventDefault()
      Stores.SplitView.setDividerWidth(leftPaneIndex, currentLeftWidth() + STEP)
    }
  }

  return (
    <div
      ref={ref}
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize panes"
      aria-valuenow={width ? Math.round(width) : undefined}
      aria-valuemin={SPLIT_LIMITS.MIN_PANE_WIDTH}
      aria-valuemax={SPLIT_LIMITS.MAX_PANE_WIDTH}
      tabIndex={0}
      data-testid={`split-divider-${leftPaneIndex}`}
      onPointerDown={onDown}
      onKeyDown={onKeyDown}
      className={cn(
        // Reads as a normal 1px border: the VISIBLE line is the `w-px` child; the
        // outer element is a wider INVISIBLE grab area, pulled back with negative
        // margins so its net layout footprint stays 1px (panes sit flush). `z-[1]`
        // lifts the grab area above both neighbour panes but below the fixed z-10
        // sidebar toggle (FB-18).
        'group relative z-[1] flex shrink-0 cursor-col-resize justify-center',
        'w-[9px] -mx-1 focus-visible:outline-none',
      )}
    >
      <div className="h-full w-px bg-border transition-colors group-hover:bg-primary/50 group-focus-visible:bg-primary" />
    </div>
  )
}
