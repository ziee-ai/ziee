import { useEffect, useState } from 'react'
import {
  App,
  Typography,
  Button,
  Dropdown,
  Empty,
  Menu,
  Spin,
  theme,
} from 'antd'
import type { MenuProps } from 'antd'
import {
  MessageOutlined,
  DeleteOutlined,
  MoreOutlined,
} from '@ant-design/icons'
import { useLocation, useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'
import {
  chatExtensionRegistry,
  useConversationMenuContributions,
} from '@/modules/chat/core/extensions'

const { Text } = Typography

// Shared styling with the LeftSidebar's Menus so the "Recent chats"
// group reads as the same surface family as Navigation / Tools.
// Keep in lockstep with `LeftSidebar.tsx::menuClass`.
const SIDEBAR_MENU_CLASS =
  '!bg-transparent !border-none ' +
  '[&_.ant-menu-item]:!h-7 [&_.ant-menu-item]:!leading-[28px] ' +
  '[&_.ant-menu-item]:!mx-2 ' +
  '[&_.ant-menu-item]:!w-[calc(100%-1rem)] ' +
  '[&_.ant-menu-item]:!pl-2 [&_.ant-menu-item]:!pr-2 ' +
  '[&_.ant-menu-item]:!py-0 ' +
  '[&_.ant-menu-item]:!rounded-md ' +
  '[&_.ant-menu-title-content]:!py-0 ' +
  '[&_.ant-menu-item-group-title]:!px-3 [&_.ant-menu-item-group-title]:!pt-0 ' +
  '[&_.ant-menu-item-group-title]:!pb-0.5 ' +
  '[&_.ant-menu-item-group-title]:!text-xs ' +
  '[&_.ant-menu-item-group-title]:!font-semibold ' +
  '[&_.ant-menu-item-group-title]:!tracking-wide'

/**
 * Sidebar list of the user's recent conversations, backed by
 * `Stores.ChatHistory.recentConversations`. Renders as an antd
 * `<Menu>` so hover / selected / focus styling matches the
 * Navigation + Tools menus above it in the sidebar.
 *
 * Click navigation routes through the `conversationHref` extension
 * hook so any cross-cutting feature can override URL resolution
 * per conversation without this widget knowing about it.
 */
export function RecentConversationsWidget() {
  const { token } = theme.useToken()
  const location = useLocation()
  const navigate = useNavigate()
  const { recentConversations, loading, isInitialized } = Stores.ChatHistory

  useEffect(() => {
    if (!isInitialized) {
      Stores.ChatHistory.__state.loadConversations()
    }
  }, [isInitialized])

  // Section header for the empty + loading states. Rendered as a standalone
  // styled heading (NOT an antd <Menu>) — an empty Menu group produces a
  // `role="menu"` with no children, which fails axe-core's
  // `aria-required-children`. The classes mirror the Menu group-title
  // typography in SIDEBAR_MENU_CLASS so it reads identically.
  const headerOnly = (
    <div
      className="px-3 pt-0 pb-0.5 text-xs font-semibold tracking-wide"
      style={{ color: token.colorTextDescription }}
    >
      Recent chats
    </div>
  )

  if (loading && !isInitialized) {
    return (
      <div className="flex flex-col h-full">
        {headerOnly}
        <div className="flex justify-center items-center py-8">
          <Spin />
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
            image={<MessageOutlined className="text-4xl text-gray-400" />}
            description={
              <Text type="secondary" className="text-xs">
                No conversations yet
              </Text>
            }
            styles={{ image: { height: 40 } }}
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
  // treatment (token-based colorPrimary background + colorText).
  const selectedKey = recentConversations.find(
    c => location.pathname === hrefFor(c),
  )?.id

  const items: MenuProps['items'] = recentConversations.map(c => ({
    key: c.id,
    label: <ConversationRowLabel conversation={c} />,
  }))

  return (
    <div
      className="flex flex-col h-full min-h-0"
      // Hold the section header outside the scroll viewport so it
      // stays put while the list scrolls — matches how the original
      // implementation pinned its bespoke header.
      style={{ color: token.colorTextBase }}
    >
      <DivScrollY className="flex-col flex-1 min-h-0">
        <Menu
          mode="inline"
          className={SIDEBAR_MENU_CLASS}
          selectedKeys={selectedKey ? [selectedKey] : []}
          items={[
            {
              type: 'group',
              label: 'Recent chats',
              children: items,
            },
          ]}
          onClick={({ key, domEvent }) => {
            // The per-row action dropdown stops propagation, so any
            // click we see here came from the row body itself.
            const c = recentConversations.find(x => x.id === key)
            if (!c) return
            // Defensive: still bail if the click originated inside the
            // floating dropdown menu (body-level portal), in case
            // antd's event routing ever changes.
            const target = domEvent.target as HTMLElement | null
            if (target?.closest('.ant-dropdown')) return
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
  const { token } = theme.useToken()
  const { modal } = App.useApp()
  const [deleting, setDeleting] = useState(false)
  // Controlled dropdown open so we can suppress closing while an
  // extension overlay (popconfirm etc.) is showing.
  const [menuOpen, setMenuOpen] = useState(false)

  const { items: extensionItems, overlays, keepMenuOpen } =
    useConversationMenuContributions(conversation)

  const confirmDelete = () => {
    const title = conversation.title || 'Untitled Conversation'
    modal.confirm({
      title: 'Delete conversation?',
      content: `"${title}" will be permanently deleted.`,
      okText: 'Delete',
      cancelText: 'Cancel',
      okButtonProps: { danger: true },
      onOk: async () => {
        setDeleting(true)
        try {
          await Stores.ChatHistory.__state.deleteConversation(conversation.id)
        } finally {
          setDeleting(false)
        }
      },
    })
  }

  const menuItems = [
    ...(extensionItems ?? []),
    ...(extensionItems && extensionItems.length > 0
      ? [{ type: 'divider' as const, key: 'div-delete' }]
      : []),
    {
      key: 'delete',
      danger: true,
      icon: <DeleteOutlined />,
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
          'row-actions flex-shrink-0 opacity-0 group-hover:opacity-100 ' +
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
          menu={{ items: menuItems }}
          trigger={['click']}
          placement="bottomRight"
          open={menuOpen || keepMenuOpen}
          onOpenChange={open => {
            if (!open && keepMenuOpen) return
            setMenuOpen(open)
          }}
        >
          <Button
            type="text"
            size="small"
            icon={<MoreOutlined />}
            loading={deleting}
            style={{
              width: 22,
              height: 22,
              padding: 0,
              color: token.colorText,
            }}
            aria-label="Conversation options"
          />
        </Dropdown>
      </div>
      {/* Extension overlays (modals, popconfirms). Render alongside
          the row trigger; menu items above toggle their state. */}
      {overlays}
    </div>
  )
}
