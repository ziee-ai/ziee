import { useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { useVirtualizer } from '@tanstack/react-virtual'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import {
  Button,
  Dropdown,
  Empty,
  ErrorState,
  Spin,
  Text,
  Tooltip,
  dialog,
  menuRowClasses,
  MenuRowButton,
} from '@ziee/kit'
import type { DropdownItem } from '@ziee/kit'
import { MessageSquare, Trash2, MoreVertical } from 'lucide-react'
import { useLocation, useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'
import {
  chatExtensionRegistry,
  useConversationMenuContributions,
} from '@/modules/chat/core/extensions'

// INITIAL height estimate for one recent-chat row: the Menu row button is
// `px-3 py-1.5 text-sm` (~32px) + a 2px inter-row gap ≈ 34px. This is only the
// pre-measure guess — each row is measured via `virt.measureElement`, so text
// zoom / a large OS font / a wrapped title grows the row correctly instead of
// clipping against a rigid grid.
const ROW_H = 34

/**
 * Sidebar list of the user's recent conversations, backed by
 * `Stores.ChatHistory.recentConversations`. INFINITE-SCROLL + VIRTUALIZED: the
 * first page loads on mount and the next page auto-loads as the last virtual row
 * nears the end (a nav-feed idiom); only the visible window is ever in the DOM,
 * so a user with thousands of chats scrolls in O(viewport). Row styling is shared
 * with the kit `<Menu>` (`menuRowClasses`) so it stays pixel-faithful to the
 * Navigation + Tools menus above it; rows are a `role="list"` of navigation
 * buttons (a virtualized `role="menu"` can't honor the ARIA menu keyboard
 * contract across non-rendered items).
 *
 * Click navigation routes through the `conversationHref` extension hook so any
 * cross-cutting feature can override URL resolution per conversation.
 */
export function RecentConversationsWidget() {
  const location = useLocation()
  const navigate = useNavigate()
  const {
    recentConversations,
    recentInitialized,
    recentTotal,
    recentHasMore,
    recentLoadingMore,
    recentError,
  } = Stores.ChatHistory

  useEffect(() => {
    if (!recentInitialized) {
      Stores.ChatHistory.loadRecentConversations()
    }
  }, [recentInitialized])

  // OverlayScrollbars viewport is the virtualizer's scroll root. `initialized`
  // flips `scrollReady` so the virtualizer re-reads `getScrollElement` once the
  // (deferred) viewport exists — mirrors `kit/table.tsx::VirtualTable`.
  const osRef = useRef<OverlayScrollbarsComponentRef>(null)
  const [, setScrollReady] = useState(false)
  const getScrollElement = useCallback(
    () => osRef.current?.osInstance()?.elements().viewport ?? null,
    [],
  )
  const virt = useVirtualizer({
    count: recentConversations.length,
    getScrollElement,
    estimateSize: () => ROW_H,
    overscan: 8,
  })
  const virtualItems = virt.getVirtualItems()

  // Auto-load the next page when the last rendered virtual row reaches the end of
  // the loaded set (the tanstack-virtual infinite-scroll idiom). The `!recentError`
  // gate is load-bearing: on a persistent load-MORE failure the store leaves
  // recentHasMore=true, so WITHOUT this gate the effect would re-fire the instant
  // recentLoadingMore flips back to false and hammer the API in a tight loop.
  const lastIndex =
    virtualItems.length > 0 ? virtualItems[virtualItems.length - 1].index : -1
  useEffect(() => {
    if (
      lastIndex >= 0 &&
      lastIndex >= recentConversations.length - 1 &&
      recentHasMore &&
      !recentLoadingMore &&
      !recentError
    ) {
      void Stores.ChatHistory.loadMoreRecent()
    }
  }, [lastIndex, recentConversations.length, recentHasMore, recentLoadingMore, recentError])

  // Explicit retry for a FAILED load-more (list already populated). A visible
  // affordance — NOT scroll-to-clear — because a loaded page that fits the
  // viewport can't be scrolled, which would otherwise strand paging silently.
  const retryLoadMore = () => {
    Stores.ChatHistory.clearRecentError()
    void Stores.ChatHistory.loadMoreRecent()
  }

  // The currently-open conversation (for the `aria-current` selected row).
  // Memoized so the virtualizer's per-scroll-frame re-renders don't re-run this
  // O(loaded-rows) scan (which calls the `conversationHref` extension hook per
  // row) — it only recomputes when the list or the URL actually changes.
  const selectedId = useMemo(() => {
    const path = location.pathname
    for (const c of recentConversations) {
      const href = chatExtensionRegistry.conversationHref(c) ?? `/chat/${c.id}`
      if (path === href) return c.id
    }
    return undefined
  }, [recentConversations, location.pathname])

  // Section header (standalone, above the scroll area) — its typography mirrors
  // the Menu group-title so it reads identically to the sections above.
  const headerOnly = (
    <div className="px-3 pt-0 pb-0.5 text-xs font-semibold tracking-wide text-muted-foreground">
      Recent chats
    </div>
  )

  // First-load failure (nothing loaded yet): show a retryable error, never wedge
  // on the spinner. A failed load-MORE (list already populated) is handled in the
  // store by just clearing the flag, so the loaded rows stay and we fall through.
  if (recentError && recentConversations.length === 0) {
    return (
      <div className="flex flex-col h-full">
        {headerOnly}
        <div className="px-2 py-4">
          <ErrorState
            data-testid="chat-recent-error"
            resource="recent chats"
            description="Your recent chats couldn't be loaded."
            details={recentError}
            onRetry={() => Stores.ChatHistory.loadRecentConversations(1)}
          />
        </div>
      </div>
    )
  }

  if (!recentInitialized) {
    return (
      <div className="flex flex-col h-full">
        {headerOnly}
        <div className="flex justify-center items-center py-8">
          <Spin label="Loading" />
        </div>
      </div>
    )
  }

  if (recentConversations.length === 0) {
    return (
      <div className="flex flex-col h-full">
        {headerOnly}
        <div className="px-2 py-4">
          <Empty
            data-testid="chat-recent-empty"
            image={<MessageSquare className="size-9 text-muted-foreground" />}
            description={
              <Text type="secondary" className="text-xs">
                No conversations yet
              </Text>
            }
          />
        </div>
      </div>
    )
  }

  // Conversation href is owned by the extension registry — the SAME resolution
  // the memoized `selectedId` above uses, so the selected row stays in lockstep
  // with where a click actually navigates.
  const hrefFor = (c: ConversationResponse) =>
    chatExtensionRegistry.conversationHref(c) ?? `/chat/${c.id}`
  // aria-setsize: once fully loaded, the exact rendered count is authoritative;
  // while more remain, recentTotal is the best (approximate) estimate. Never
  // report a set size smaller than what's rendered.
  const setSize = recentHasMore
    ? Math.max(recentTotal, recentConversations.length)
    : recentConversations.length

  return (
    <div className="flex flex-col h-full min-h-0 text-foreground">
      {headerOnly}
      <DivScrollY
        ref={osRef}
        className="flex-col flex-1 min-h-0 px-2"
        events={{ initialized: () => setScrollReady(true) }}
      >
        {/* role="list" (not menu): the window is virtualized, so the ARIA menu
            keyboard contract across non-rendered items can't be honored. Rows are
            navigation buttons; aria-setsize/posinset restore list position. */}
        <ul
          role="list"
          aria-label="Recent conversations"
          data-testid="chat-recent-conversations-list"
          className="relative w-full m-0 p-0 list-none"
          style={{ height: virt.getTotalSize() }}
        >
          {virtualItems.map(vi => {
            const c = recentConversations[vi.index]
            if (!c) return null
            const title = c.title || 'Untitled Conversation'
            const selected = c.id === selectedId
            const cls = menuRowClasses({ selected, hasActions: true })
            return (
              <li
                key={c.id}
                ref={virt.measureElement}
                data-index={vi.index}
                // pb-0.5 reproduces the kit Menu's 2px inter-row gap; the row is
                // content-height (measured), not forced to a rigid ROW_H, so it
                // grows with text zoom instead of clipping.
                className="absolute start-0 w-full pb-0.5"
                style={{ top: 0, transform: `translateY(${vi.start}px)` }}
                aria-setsize={setSize}
                aria-posinset={vi.index + 1}
              >
                {/* The row-style container carries `relative group/menu-row` so the
                    trailing actions overlay anchors here (the <li> is positioned by
                    the virtualizer). */}
                <div className={cls.row}>
                  <MenuRowButton
                    selected={selected}
                    data-testid={`chat-recent-conversations-menu-item-${c.id}`}
                    title={title}
                    onClick={() => {
                      // Bail if the click originated inside a floating dropdown
                      // (body-level portal), in case event routing ever changes.
                      const active = document.activeElement as HTMLElement | null
                      if (active?.closest('[role="menu"]')) return
                      navigate(hrefFor(c))
                    }}
                  >
                    <span className="min-w-0 flex-1 truncate text-start">
                      {title}
                    </span>
                  </MenuRowButton>
                  <div className={cls.actions}>
                    <ConversationRowActions conversation={c} />
                  </div>
                </div>
              </li>
            )
          })}
        </ul>
        {recentLoadingMore && (
          // The Spin's inner Spinner is the single role="status" live region (no
          // outer aria-live here → no duplicate announcement); `description` gives
          // sighted users a visible caption, not just a bare icon.
          <div
            data-testid="chat-recent-loading-more"
            className="flex justify-center items-center py-3"
          >
            <Spin
              size="sm"
              label="Loading more conversations"
              description="Loading more…"
            />
          </div>
        )}
        {recentError && !recentLoadingMore && recentConversations.length > 0 && (
          // A FAILED load-more (the list is populated, so the empty-state
          // ErrorState above doesn't apply): show an inline retry so paging is
          // recoverable even when the loaded page fits the viewport (no scroll).
          <div
            data-testid="chat-recent-loadmore-error"
            className="flex justify-center items-center gap-2 py-3"
          >
            <Text type="secondary" className="text-xs">
              Couldn't load more.
            </Text>
            <Button
              data-testid="chat-recent-loadmore-retry"
              variant="ghost"
              className="h-7 px-2 text-xs"
              onClick={retryLoadMore}
            >
              Retry
            </Button>
          </div>
        )}
      </DivScrollY>
    </div>
  )
}

/**
 * Renders one Menu row's hover-only actions button (a sibling of the row's
 * navigate <button>, NOT a child of it — nesting a <button> in a <button> is
 * invalid HTML). The actions button hosts a dropdown with extension
 * contributions (project: open/add/remove, future: …) and the always-present
 * Delete entry.
 *
 * The wrapper has `onClick={e => e.stopPropagation()}` so opening the dropdown
 * does NOT bubble up to any ancestor row handler.
 */
function ConversationRowActions({
  conversation,
}: {
  conversation: ConversationResponse
}) {
  const [deleting, setDeleting] = useState(false)
  // Controlled dropdown open so we can suppress closing while an
  // extension overlay (popconfirm etc.) is showing.
  const [menuOpen, setMenuOpen] = useState(false)

  const { items: extensionItems, overlays, keepMenuOpen } =
    useConversationMenuContributions(conversation)

  const confirmDelete = async () => {
    const title = conversation.title || 'Untitled Conversation'
    const ok = await dialog.confirm({
      title: 'Delete conversation?',
      description: `"${title}" will be permanently deleted.`,
      okText: 'Delete',
      cancelText: 'Cancel',
      danger: true,
      okTestId: 'chat-conversation-delete-confirm-btn',
    })
    if (ok) {
      setDeleting(true)
      try {
        await Stores.ChatHistory.deleteConversation(conversation.id)
      } finally {
        setDeleting(false)
      }
    }
  }

  const menuItems: DropdownItem[] = [
    ...(extensionItems ?? []),
    ...(extensionItems && extensionItems.length > 0
      ? [{ type: 'divider' as const, key: 'div-delete' }]
      : []),
    {
      key: 'delete',
      danger: true,
      icon: <Trash2 />,
      label: 'Delete',
      onClick: confirmDelete,
    },
  ]

  // The `group/menu-row` group lives on the Menu row <li> (see the Menu `actions`
  // slot); these actions fade in on row hover/focus without a stateful onMouseEnter.
  return (
    <div
      className={
        // pointer-events mirror the opacity reveal: the parent Menu mask is
        // pointer-events-none, so the kebab must re-enable its own events when
        // shown (and stay inert while hidden so row clicks pass through it).
        'row-actions flex-shrink-0 opacity-0 pointer-events-none group-hover/menu-row:opacity-100 group-hover/menu-row:pointer-events-auto group-focus-within/menu-row:opacity-100 group-focus-within/menu-row:pointer-events-auto hover-none:opacity-100 hover-none:pointer-events-auto ' +
        'transition-opacity duration-150'
      }
      // Keep the button visible + interactive while its dropdown is open OR while
      // a delete is in flight — `opacity-0` / pointer-events-none would otherwise
      // hide it mid-interaction. Inline style wins over the Tailwind classes.
      style={
        menuOpen || keepMenuOpen || deleting
          ? { opacity: 1, pointerEvents: 'auto' }
          : undefined
      }
      onClick={e => e.stopPropagation()}
    >
      {/* One styled tooltip only: put the kit Tooltip on the span (not the
          Button) so its trigger is a DIFFERENT node from the Dropdown trigger,
          AND set data-tooltip-wrapped on the Button to kill its own auto-tooltip
          (icon-only + aria-label). Two overlapping Base-UI tooltips is what
          thrashed; a single one on a sibling node coexists with the menu. */}
      <Tooltip title="Conversation options">
        <span className="inline-flex">
          <Dropdown
            data-testid={`chat-recent-row-menu-${conversation.id}`}
            items={menuItems}
            side="bottom"
            align="end"
            open={menuOpen || keepMenuOpen}
            onOpenChange={open => {
              if (!open && keepMenuOpen) return
              setMenuOpen(open)
            }}
          >
            <Button
              data-testid={`chat-recent-row-actions-btn-${conversation.id}`}
              variant="ghost"
              size="icon"
              icon={<MoreVertical />}
              loading={deleting}
              className="w-[22px] h-[22px] p-0"
              aria-label="Conversation options"
              data-tooltip-wrapped=""
            />
          </Dropdown>
        </span>
      </Tooltip>
      {/* Extension overlays (modals, popconfirms). Render alongside
          the row trigger; menu items above toggle their state. */}
      {overlays}
    </div>
  )
}
