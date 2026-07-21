import { Fragment, useEffect, useRef } from 'react'
import { cn } from '@/lib/utils'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { useWindowMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { ChatPaneProvider } from '@/modules/chat/core/pane/ChatPaneContext'
import { ConversationPane } from '@/modules/chat/pages/ConversationPage'
import { SPLIT_LIMITS } from '@/modules/chat/core/split/limits'
import { AppLayout } from '@/modules/layouts/app-layout/appLayout'
import { SplitView as SplitViewStore } from '@/modules/chat/core/stores/splitView'

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
  const { md } = useWindowMinSize()
  // SplitChatView OWNS the native-scroll flag for the whole mobile split (FB-26):
  // on a phone (`useNativeScroll` self-gates on `xs`, so `md` here only bites on a
  // phone — tablet/desktop are no-ops) the single visible pane scrolls the WINDOW
  // like a normal single-pane conversation (auto-hiding header). Owning it in ONE
  // place — not per-pane — means switching the focused pane never races the global
  // flag. We READ the flag back to relax this shell so the pane content can extend
  // the document.
  useNativeScroll(md)
  const nativeScroll = AppLayout.nativeScroll
  const { panes, focusedPaneId, dividerWidths } = SplitViewStore
  // At/below `md` (≤768px) there isn't room to tile columns → single-visible-pane
  // mode (ITEM-30 / FB-26): ONE pane shows full-width; the others stay MOUNTED but
  // `hidden` so a background pane keeps streaming. There is NO tab strip and NO
  // drag chrome on small screens — switching / adding / closing panes happens in
  // the `PaneManagerDrawer` (opened from the focused pane's "Panes" button), and
  // the focused pane reads as a normal single-pane conversation. `useWindowMinSize().md`
  // (read once above) is TRUE when the viewport is AT MOST 768px (mobile/tablet), so
  // this branch is `if (md)`; desktop (md === false) tiles columns.

  // ONE tree for both modes (not two branches): crossing the `md` breakpoint must
  // NOT change a pane's element TYPE or its key, or React would unmount + remount
  // every `ChatPaneProvider` (recreating the per-pane store → tearing down live
  // streams). Each pane is always `<Fragment key=paneId><div key="pane">`; the
  // divider (columns-only) toggles around it, and the KEYED children keep the pane
  // div reconciled by key regardless (DRIFT-2.14).
  return (
    <div
      className={cn(
        'flex w-full',
        md
          ? // Mobile: stack. When the focused pane is on native document-scroll,
            // DON'T clip/fix-height — let its content scroll the window (auto-hide
            // header, FB-26). Else (tablet, non-xs) keep the inner-scroll shell.
            nativeScroll
            ? 'flex-col'
            : 'flex-col h-full min-h-0 overflow-hidden'
          : 'h-full min-h-0 overflow-hidden',
      )}
      data-testid="split-chat-view"
      data-split-mode={md ? 'tabs' : 'columns'}
    >
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
              'relative flex-col',
              md
                ? p.paneId === focusedPaneId
                  ? // Focused mobile pane: on native scroll, flow (no clip) so its
                    // content extends the document; else fill the inner shell.
                    nativeScroll
                    ? 'flex w-full'
                    : 'flex flex-1 min-h-0 overflow-hidden'
                  : 'hidden'
                : cn(
                    'flex min-w-0 min-h-0 overflow-hidden transition-opacity duration-200',
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
            onPointerDownCapture={() => SplitViewStore.focusPane(p.paneId)}
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
  const width = SplitViewStore.dividerWidths[leftPaneIndex]

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
      if (d) SplitViewStore.setDividerWidth(idxRef.current, d.last)
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
      SplitViewStore.setDividerWidth(leftPaneIndex, currentLeftWidth() - STEP)
    } else if (e.key === 'ArrowRight') {
      e.preventDefault()
      SplitViewStore.setDividerWidth(leftPaneIndex, currentLeftWidth() + STEP)
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
