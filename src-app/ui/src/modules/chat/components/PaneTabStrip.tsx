import { Button } from '@/components/ui'
import { X } from 'lucide-react'
import { cn } from '@/lib/utils'
import { Stores } from '@/core'
import { useClosePane } from '@/modules/chat/core/pane/useOpenConversation'
import type { Pane } from '@/modules/chat/core/stores/SplitView.store'

/**
 * Mobile / narrow-viewport tab strip (ITEM-30). Below the `md` breakpoint the
 * split can't tile columns, so `SplitChatView` renders ONE visible pane plus this
 * strip — a tab per open conversation. Tapping a tab focuses that pane (which is
 * the one `SplitChatView` shows); the ✕ closes it. All panes stay MOUNTED (only
 * the focused one is visible), so a background pane keeps streaming.
 */
export function PaneTabStrip() {
  const { panes, focusedPaneId } = Stores.SplitView
  const closePane = useClosePane()

  return (
    <div
      role="tablist"
      aria-label="Open conversations"
      data-testid="pane-tab-strip"
      className="flex shrink-0 items-center gap-1 overflow-x-auto border-b bg-muted/30 px-2 py-1"
    >
      {panes.map((p, i) => (
        <PaneTab
          key={p.paneId}
          pane={p}
          index={i}
          active={p.paneId === focusedPaneId}
          onClose={() => closePane(p.paneId)}
        />
      ))}
    </div>
  )
}

function PaneTab({
  pane,
  index,
  active,
  onClose,
}: {
  pane: Pane
  index: number
  active: boolean
  onClose: () => void
}) {
  // Titles for non-focused panes aren't reachable from their own (per-pane)
  // stores here, so resolve from the shared conversation list; fall back to a
  // generic label. An empty pane is the picker → "New chat".
  const { conversations, recentConversations } = Stores.ChatHistory
  const title = pane.conversationId
    ? [...recentConversations, ...conversations].find(
        (c) => c.id === pane.conversationId,
      )?.title || 'Conversation'
    : 'New chat'

  return (
    <div
      className={cn(
        'flex shrink-0 items-center rounded-md',
        active ? 'bg-background ring-1 ring-border' : 'hover:bg-accent',
      )}
    >
      <Button
        variant="ghost"
        role="tab"
        aria-selected={active}
        data-testid={`pane-tab-${index}`}
        className="h-7 max-w-[10rem] justify-start truncate px-2 font-normal"
        onClick={() => Stores.SplitView.focusPane(pane.paneId)}
      >
        <span className="truncate">{title}</span>
      </Button>
      <Button
        variant="ghost"
        size="icon"
        icon={<X />}
        aria-label="Close pane"
        data-testid={`pane-tab-close-${index}`}
        className="size-6"
        onClick={onClose}
      />
    </div>
  )
}
