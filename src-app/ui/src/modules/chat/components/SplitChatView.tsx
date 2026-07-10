import { Fragment, useRef } from 'react'
import { cn } from '@/lib/utils'
import { Stores } from '@/core'
import { useNativeScroll } from '@/modules/layouts/app-layout/hooks/useNativeScroll'
import { ChatPaneProvider } from '@/modules/chat/core/pane/ChatPaneContext'
import { ConversationPane } from '@/modules/chat/pages/ConversationPage'
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

  return (
    <div
      className="flex h-full min-h-0 w-full overflow-hidden"
      data-testid="split-chat-view"
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

  const onMove = (e: PointerEvent) => {
    if (!drag.current) return
    Stores.SplitView.setDividerWidth(
      leftPaneIndex,
      drag.current.w + (e.clientX - drag.current.x),
    )
  }
  const onUp = () => {
    drag.current = null
    window.removeEventListener('pointermove', onMove)
    window.removeEventListener('pointerup', onUp)
  }
  const onDown = (e: React.PointerEvent) => {
    const leftEl = ref.current?.previousElementSibling as HTMLElement | null
    const w =
      leftEl?.getBoundingClientRect().width ?? SPLIT_LIMITS.MIN_PANE_WIDTH
    drag.current = { x: e.clientX, w }
    window.addEventListener('pointermove', onMove)
    window.addEventListener('pointerup', onUp)
  }

  return (
    <div
      ref={ref}
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize panes"
      data-testid={`split-divider-${leftPaneIndex}`}
      onPointerDown={onDown}
      className="w-1 shrink-0 cursor-col-resize bg-border transition-colors hover:bg-primary/50"
    />
  )
}
