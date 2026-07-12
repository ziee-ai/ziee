import { useEffect, useState } from 'react'
import {
  Button,
  Dropdown,
  Empty,
  Spin,
  Text,
  Tooltip,
  dialog,
} from '@/components/ui'
import type { DropdownItem } from '@/components/ui'
import { Columns2, MessageSquare, Trash2, MoreVertical } from 'lucide-react'
import { useLocation } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'
import { Menu } from '@/components/ui'
import type { MenuItem } from '@/components/ui'
import {
  chatExtensionRegistry,
  useConversationMenuContributions,
} from '@/modules/chat/core/extensions'
import { useOpenConversationInWorkspace } from '@/modules/chat/core/pane/useOpenConversation'
import { setConversationDragData } from '@/modules/chat/core/pane/paneDnd'
import { useConversationTearOff } from '@/modules/chat/core/popout/useConversationTearOff'

const RECENT_ITEM_TESTID_PREFIX = 'chat-recent-conversations-menu-item-'

/**
 * Sidebar list of the user's recent conversations, backed by
 * `Stores.ChatHistory.recentConversations`. Renders as a kit
 * `<Menu>` so hover / selected / focus styling matches the
 * Navigation + Tools menus above it in the sidebar.
 *
 * Click navigation routes through the `conversationHref` extension
 * hook so any cross-cutting feature can override URL resolution
 * per conversation without this widget knowing about it.
 */
export function RecentConversationsWidget() {
  const location = useLocation()
  const openConversation = useOpenConversationInWorkspace()
  // Tear-off (ITEM-58): releasing a sidebar drag past the window edge pops the
  // conversation into its own desktop window (no-op on web / in-window).
  const tearOff = useConversationTearOff()
  const { recentConversations, loading, isInitialized } = Stores.ChatHistory

  // Cmd/Ctrl/middle-click a row → open it in a NEW pane (ITEM-28) instead of the
  // plain navigate the Menu's onSelect does. Handled in the capture phase so a
  // modified click never reaches the row button's onSelect (which would navigate);
  // the row button carries `${prefix}<id>` as its testid, an ancestor of any
  // click target inside the row, so it resolves reliably.
  const openInNewPaneIfModified = (
    e: React.MouseEvent<HTMLDivElement>,
  ): void => {
    if (!(e.metaKey || e.ctrlKey || e.button === 1)) return
    const row = (e.target as HTMLElement).closest<HTMLElement>(
      `[data-testid^="${RECENT_ITEM_TESTID_PREFIX}"]`,
    )
    const id = row?.getAttribute('data-testid')?.slice(RECENT_ITEM_TESTID_PREFIX.length)
    const c = id && recentConversations.find((x) => x.id === id)
    if (!c) return
    e.preventDefault()
    e.stopPropagation()
    openConversation(c.id, { intent: 'newPane', href: hrefFor(c) })
  }

  useEffect(() => {
    if (!isInitialized) {
      Stores.ChatHistory.loadConversations()
    }
  }, [isInitialized])

  // Section header for the empty + loading states. Rendered as a standalone
  // styled heading (NOT a Menu) — an empty Menu group produces a
  // `role="menu"` with no children, which fails axe-core's
  // `aria-required-children`. The classes mirror the Menu group-title
  // typography so it reads identically.
  const headerOnly = (
    <div
      className="px-3 pt-0 pb-0.5 text-xs font-semibold tracking-wide text-muted-foreground"
    >
      Recent chats
    </div>
  )

  if (loading && !isInitialized) {
    return (
      <div className="flex flex-col h-full">
        {headerOnly}
        <div className="flex justify-center items-center py-8">
          <Spin label="Loading" />
        </div>
      </div>
    )
  }

  if (!loading && recentConversations.length === 0) {
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

  // Conversation href is owned by the extension registry — same call
  // the row click handler uses below, so the selected-key derivation
  // stays in lockstep with what navigation actually does.
  const hrefFor = (c: ConversationResponse) =>
    chatExtensionRegistry.conversationHref(c) ?? `/chat/${c.id}`

  // The currently-open conversation gets the Menu's `selected`
  // treatment (bg-accent + font-medium).
  const selectedKey = recentConversations.find(
    c => location.pathname === hrefFor(c),
  )?.id

  const items: MenuItem[] = [
    {
      type: 'group',
      label: 'Recent chats',
      children: recentConversations.map(c => {
        const title = c.title || 'Untitled Conversation'
        return {
          key: c.id,
          title,
          label: (
            // Drag source (ITEM-31): drag a conversation onto a pane header
            // (replace) or the inter-pane seam (new pane).
            <span
              className="truncate"
              title={title}
              draggable
              onDragStart={e => setConversationDragData(e.dataTransfer, c.id)}
              onDragEnd={e => tearOff(e, { conversationId: c.id, title })}
            >
              {title}
            </span>
          ),
          // Actions render as a SIBLING of the row button (see Menu `actions`) —
          // a <button> may not contain the dropdown's own <button>.
          actions: <ConversationRowActions conversation={c} />,
        }
      }),
    },
  ]

  return (
    <div
      className="flex flex-col h-full min-h-0 text-foreground"
      onClickCapture={openInNewPaneIfModified}
      onAuxClickCapture={openInNewPaneIfModified}
    >
      <DivScrollY className="flex-col flex-1 min-h-0">
        <Menu
          data-testid="chat-recent-conversations-menu"
          mode="vertical"
          aria-label="Recent conversations"
          className="px-2"
          items={items}
          selectedKey={selectedKey}
          onSelect={key => {
            const c = recentConversations.find(x => x.id === key)
            if (!c) return
            // Defensive: bail if the click originated inside a floating
            // dropdown menu (body-level portal), in case event routing
            // ever changes.
            const active = document.activeElement as HTMLElement | null
            if (active?.closest('[role="menu"]')) return
            // Route through the workspace reducer (ITEM-28): plain click replaces
            // the focused pane while split, else a normal single-pane navigate.
            openConversation(c.id, { intent: 'auto', href: hrefFor(c) })
          }}
        />
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
  const openConversation = useOpenConversationInWorkspace()

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
    {
      key: 'open-in-split',
      icon: <Columns2 />,
      label: 'Open in split pane',
      onClick: () =>
        openConversation(conversation.id, {
          intent: 'newPane',
          href:
            chatExtensionRegistry.conversationHref(conversation) ??
            `/chat/${conversation.id}`,
        }),
    },
    ...(extensionItems ?? []),
    { type: 'divider' as const, key: 'div-delete' },
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
