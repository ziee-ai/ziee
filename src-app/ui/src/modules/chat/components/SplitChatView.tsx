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
  // Below `md` there isn't room to tile columns → tab mode (ITEM-30): one visible
  // pane + a tab strip. All panes stay MOUNTED (only the focused one is shown) so
  // a background pane keeps streaming.
  const { md } = useWindowMinSize()

  if (!md) {
    return (
      <div
        className="flex h-full min-h-0 w-full flex-col overflow-hidden"
        data-testid="split-chat-view"
        data-split-mode="tabs"
      >
        <PaneTabStrip />
        {panes.map((p, i) => (
          <div
            key={p.paneId}
            role="tabpanel"
            data-testid={`chat-pane-${i}`}
            className={cn(
              'relative min-h-0 flex-1 flex-col overflow-hidden',
              p.paneId === focusedPaneId ? 'flex' : 'hidden',
            )}
          >
            <ChatPaneProvider paneId={p.paneId} conversationId={p.conversationId}>
              <ConversationPane />
            </ChatPaneProvider>
          </div>
        ))}
      </div>
    )
  }

  return (
    <div
      className="flex h-full min-h-0 w-full overflow-hidden"
      data-testid="split-chat-view"
      data-split-mode="columns"
    >
      {panes.map((p, i) => (
        <Fragment key={p.paneId}>
          {i > 0 && <SplitDivider leftPaneIndex={i - 1} />}
          {/* biome-ignore lint/a11y/useKeyWithClickEvents: focus-on-interact is
              pointer-scoped; keyboard focus already lands via the pane's own
              focusable controls, and pointerDownCapture never steals it. */}
          <div
            data-testid={`chat-pane-${i}`}
            className={cn(
              'relative flex min-w-0 flex-col overflow-hidden',
              focusedPaneId === p.paneId &&
                'z-10 ring-2 ring-primary ring-inset',
            )}
            style={{
              flex: dividerWidths[i]
                ? `0 0 ${dividerWidths[i]}px`
                : '1 1 0%',
            }}
            onPointerDownCapture={() => Stores.SplitView.focusPane(p.paneId)}
          >
            <ChatPaneProvider
              paneId={p.paneId}
              conversationId={p.conversationId}
            >
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
  const drag = useRef<{ x: number; w: number } | null>(null)
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
      if (!drag.current) return
      Stores.SplitView.setDividerWidth(
        idxRef.current,
        drag.current.w + (e.clientX - drag.current.x),
      )
    },
    up: () => {
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
    drag.current = { x: e.clientX, w: currentLeftWidth() }
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
      className="w-1 shrink-0 cursor-col-resize bg-border transition-colors hover:bg-primary/50 focus-visible:bg-primary focus-visible:outline-none"
    />
  )
}
