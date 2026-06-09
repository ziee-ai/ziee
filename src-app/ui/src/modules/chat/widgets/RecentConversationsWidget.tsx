import { useEffect, useRef, useState } from 'react'
import {
  App,
  Typography,
  Button,
  Dropdown,
  Empty,
  Spin,
  theme,
} from 'antd'
import {
  MessageOutlined,
  DeleteOutlined,
  MoreOutlined,
} from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'
import {
  chatExtensionRegistry,
  useConversationMenuContributions,
} from '@/modules/chat/core/extensions'

const { Text } = Typography

/**
 * Sidebar list of the user's recent conversations, backed by
 * `Stores.ChatHistory.recentConversations`.
 *
 * Click navigation routes through the `conversationHref` extension
 * hook so any cross-cutting feature can override URL resolution
 * per conversation without this widget knowing about it.
 */
export function RecentConversationsWidget() {
  const { token } = theme.useToken()
  const {
    recentConversations,
    loading,
    isInitialized,
  } = Stores.ChatHistory

  useEffect(() => {
    if (!isInitialized) {
      Stores.ChatHistory.__state.loadConversations()
    }
  }, [isInitialized])

  // Section header matching the LeftSidebar's `SectionHeader` style.
  const header = (
    <div className="flex-shrink-0">
      <Text
        className="px-3 pb-0.5 block font-semibold tracking-wide"
        style={{
          fontSize: token.fontSizeSM,
          color: token.colorTextSecondary,
        }}
      >
        Recent chats
      </Text>
    </div>
  )

  if (loading && !isInitialized) {
    return (
      <div className="flex flex-col h-full">
        {header}
        <div className="flex justify-center items-center py-8">
          <Spin />
        </div>
      </div>
    )
  }

  if (!loading && recentConversations.length === 0) {
    return (
      <div className="flex flex-col h-full">
        {header}
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

  return (
    <div className="flex flex-col h-full min-h-0">
      {header}
      <DivScrollY className="flex-col flex-1 min-h-0">
        {recentConversations.map((conversation: ConversationResponse) => (
          <RecentConversationRow
            key={conversation.id}
            conversation={conversation}
          />
        ))}
      </DivScrollY>
    </div>
  )
}

/**
 * Per-row component — extracted because `useConversationMenuContributions`
 * is a hook and must be called from the top of a component (not
 * inside a `.map`).
 */
function RecentConversationRow({
  conversation,
}: {
  conversation: ConversationResponse
}) {
  const navigate = useNavigate()
  const { token } = theme.useToken()
  const { modal } = App.useApp()
  const [hovered, setHovered] = useState(false)
  const [deleting, setDeleting] = useState(false)
  // Controlled dropdown open so we can suppress closing while an
  // extension overlay (popconfirm etc.) is showing.
  const [menuOpen, setMenuOpen] = useState(false)
  const rowRef = useRef<HTMLDivElement>(null)

  // Extension contributions (project: open/add/remove, future: …).
  const {
    items: extensionItems,
    overlays,
    keepMenuOpen,
  } = useConversationMenuContributions(conversation)

  // Row-click navigates ONLY when the click landed on something
  // inside this row's DOM. Antd Dropdown's popup renders in a
  // body-level portal — clicks there have a DOM target outside
  // `rowRef`, so this check rejects them even though React's
  // synthetic event still bubbles up to here.
  const handleRowClick = (e: React.MouseEvent) => {
    if (!rowRef.current?.contains(e.target as Node)) return
    const href =
      chatExtensionRegistry.conversationHref(conversation) ??
      `/chat/${conversation.id}`
    navigate(href)
  }

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

  return (
    <div
      ref={rowRef}
      className="group relative px-3 py-1 mx-2 cursor-pointer rounded-md"
      style={{
        backgroundColor: hovered ? token.colorPrimaryHover : 'transparent',
        color: hovered ? token.colorTextLightSolid : token.colorTextBase,
        transition: 'background-color 150ms, color 150ms',
      }}
      onClick={handleRowClick}
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <Text
        className="text-sm truncate block"
        style={{ color: 'inherit' }}
        title={conversation.title || 'Untitled Conversation'}
      >
        {conversation.title || 'Untitled Conversation'}
      </Text>

      <div
        className="absolute right-2 top-1/2 -translate-y-1/2"
        style={{ width: 24, height: 24 }}
        onClick={e => e.stopPropagation()}
      >
        <Dropdown
          menu={{ items: menuItems }}
          trigger={['click']}
          placement="bottomRight"
          // Controlled open so we can keep the dropdown visible
          // while an extension overlay (popconfirm in a body-level
          // portal) is showing — clicking the overlay would
          // otherwise register as outside the dropdown and close
          // it, yanking the popconfirm's anchor away.
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
              width: 24,
              height: 24,
              padding: 0,
              backgroundColor: token.colorBgContainer,
              color: token.colorTextBase,
              border: `1px solid ${token.colorBorderSecondary}`,
              opacity: hovered || deleting ? 1 : 0,
              transition: 'opacity 120ms ease-out',
            }}
            aria-label="Conversation options"
          />
        </Dropdown>
      </div>

      {/* Extension overlays (modals, popconfirms). Mounted alongside
          the row trigger; menu items above toggle their state. */}
      {overlays}
    </div>
  )
}
