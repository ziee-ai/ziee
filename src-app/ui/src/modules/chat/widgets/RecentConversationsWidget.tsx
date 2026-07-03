import { useEffect, useState } from 'react'
import {
  Button,
  Dropdown,
  Empty,
  Spin,
  Text,
  dialog,
} from '@/components/ui'
import type { DropdownItem } from '@/components/ui'
import { MessageSquare, Trash2, MoreVertical } from 'lucide-react'
import { useLocation, useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'
import { Menu } from '@/components/ui'
import type { MenuItem } from '@/components/ui'
import {
  chatExtensionRegistry,
  useConversationMenuContributions,
} from '@/modules/chat/core/extensions'

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
  const navigate = useNavigate()
  const { recentConversations, loading, isInitialized } = Stores.ChatHistory

  useEffect(() => {
    if (!isInitialized) {
      Stores.ChatHistory.__state.loadConversations()
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
      children: recentConversations.map(c => ({
        key: c.id,
        label: <ConversationRowLabel conversation={c} />,
      })),
    },
  ]

  return (
    <div className="flex flex-col h-full min-h-0 text-foreground">
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
            navigate(hrefFor(c))
          }}
        />
      </DivScrollY>
    </div>
  )
}

/**
 * Renders one Menu item's label: the conversation title + a hover-only
 * actions button anchored to the right. The actions button hosts a
 * dropdown with extension contributions (project: open/add/remove,
 * future: …) and the always-present Delete entry.
 *
 * The button has `onClick={e => e.stopPropagation()}` so opening the
 * dropdown does NOT also fire the Menu's row-click navigate.
 */
function ConversationRowLabel({
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
        await Stores.ChatHistory.__state.deleteConversation(conversation.id)
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

  // `group` + `[&:hover_.row-actions]:opacity-100` makes the actions
  // button fade in on row hover without a stateful onMouseEnter dance.
  return (
    <div className="group flex items-center justify-between gap-2">
      <span
        className="truncate"
        title={conversation.title || 'Untitled Conversation'}
      >
        {conversation.title || 'Untitled Conversation'}
      </span>
      <div
        className={
          'row-actions flex-shrink-0 opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 hover-none:opacity-100 ' +
          'transition-opacity duration-150'
        }
        // Keep the button visible while its dropdown is open OR while
        // a delete is in flight — `opacity-0` would otherwise hide it
        // mid-interaction. Inline style wins over the Tailwind class
        // because it sets the same property.
        style={
          menuOpen || keepMenuOpen || deleting ? { opacity: 1 } : undefined
        }
        onClick={e => e.stopPropagation()}
      >
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
            tooltip="Conversation options"
          />
        </Dropdown>
      </div>
      {/* Extension overlays (modals, popconfirms). Render alongside
          the row trigger; menu items above toggle their state. */}
      {overlays}
    </div>
  )
}
