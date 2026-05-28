import { useEffect, useRef, useState } from 'react'
import {
  Typography,
  Button,
  Dropdown,
  Empty,
  Modal,
  Spin,
  theme,
} from 'antd'
import {
  MessageOutlined,
  DeleteOutlined,
  MoreOutlined,
} from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'

const { Text } = Typography

interface RecentConversationsWidgetProps {
  /**
   * Optional project-membership filter:
   *   - undefined (default): "all conversations" — uses the shared
   *     ChatHistory store like before.
   *   - `null`: only conversations NOT in any project ("unfiled").
   *     Bypasses the store and fetches directly with `?project_id=null`.
   *   - `<uuid>`: only conversations in that project. Bypasses the
   *     store and fetches directly with `?project_id=<uuid>`.
   *
   * When set (null or uuid), the widget self-manages its data so the
   * project-detail page and the sidebar's "Recent — unfiled" panel
   * don't have to share the global conversation list.
   */
  projectIdFilter?: string | null
}

/**
 * RecentConversationsWidget — sidebar list of recent conversations.
 *
 * Two modes:
 *   1. Unfiltered (default): consumes Stores.ChatHistory.recentConversations.
 *   2. Filtered (`projectIdFilter` prop set): self-fetches via the new
 *      `?project_id=null|<uuid>` filter on the conversations list endpoint.
 *      Subscribes to `conversation.created` so a new chat created in the
 *      same scope shows up without a manual reload.
 */
export function RecentConversationsWidget({
  projectIdFilter,
}: RecentConversationsWidgetProps = {}) {
  const navigate = useNavigate()
  const { token } = theme.useToken()
  const { recentConversations: storeRecent, loading: storeLoading, isInitialized: storeInitialized } = Stores.ChatHistory
  const [deletingId, setDeletingId] = useState<string | null>(null)
  const [hoveredId, setHoveredId] = useState<string | null>(null)

  // Self-managed state used only when projectIdFilter is set.
  const [localConvs, setLocalConvs] = useState<ConversationResponse[]>([])
  const [localLoading, setLocalLoading] = useState(false)
  const [localInitialized, setLocalInitialized] = useState(false)
  /// Monotonic fetch ID — closes audit N9. If `projectIdFilter`
  /// changes mid-fetch, the in-flight request's id no longer matches
  /// `latestFetchIdRef.current`, so its late-landing response is
  /// dropped instead of stomping the new filter's data.
  const latestFetchIdRef = useRef(0)

  // Default-mode mount fetch (unchanged behavior).
  useEffect(() => {
    if (projectIdFilter !== undefined) return
    if (!storeInitialized) {
      Stores.ChatHistory.__state.loadConversations()
    }
  }, [storeInitialized, projectIdFilter])

  // Filtered-mode self fetch. Re-runs when the filter changes.
  useEffect(() => {
    if (projectIdFilter === undefined) return
    let cancelled = false
    // Bump the fetch id. Any in-flight fetch whose id no longer
    // matches `latestFetchIdRef.current` will discard its result.
    latestFetchIdRef.current += 1
    const myFetchId = latestFetchIdRef.current
    setLocalLoading(true)
    ;(async () => {
      try {
        const params: any = { limit: 20, page: 1 }
        // `project_id=null` (literal string) is honored by the backend
        // to mean "unfiled only"; a UUID restricts to that project.
        params.project_id = projectIdFilter === null ? 'null' : projectIdFilter
        const resp = await ApiClient.Conversation.list(params)
        // Only commit if this is still the most recent fetch AND the
        // effect hasn't been cleaned up yet. Closes audit N9.
        if (!cancelled && latestFetchIdRef.current === myFetchId) {
          setLocalConvs(resp ?? [])
          setLocalInitialized(true)
        }
      } catch (err) {
        console.warn('[RecentConversationsWidget] filtered fetch failed', err)
      } finally {
        if (!cancelled && latestFetchIdRef.current === myFetchId) {
          setLocalLoading(false)
        }
      }
    })()

    // Refresh on conversation.created so a newly-created conversation
    // in this scope appears immediately. Strict equality on project_id
    // (so a "no project" widget doesn't show project conversations,
    // and vice versa).
    const offCreated = Stores.EventBus.on(
      'conversation.created',
      async event => {
        const c = event.data.conversation
        const matches =
          projectIdFilter === null
            ? !c.project_id
            : c.project_id === projectIdFilter
        if (matches) {
          // The event carries the bare `Conversation` (no message_count).
          // ConversationResponse adds message_count; pad with 0 so the
          // widget's render contract is uniform across both modes.
          setLocalConvs(prev => [{ ...c, message_count: 0 }, ...prev])
        }
      },
      'RecentConversationsWidget',
    )

    // F5: drop the row when ANY component deletes a conversation —
    // not just this widget's own delete-button click.
    const offDeleted = Stores.EventBus.on(
      'conversation.deleted',
      async event => {
        setLocalConvs(prev =>
          prev.filter(c => c.id !== event.data.conversationId),
        )
      },
      'RecentConversationsWidget',
    )

    return () => {
      cancelled = true
      offCreated()
      offDeleted()
    }
  }, [projectIdFilter])

  const conversations =
    projectIdFilter === undefined ? storeRecent : localConvs
  const loading =
    projectIdFilter === undefined ? storeLoading : localLoading
  const isInitialized =
    projectIdFilter === undefined ? storeInitialized : localInitialized

  const handleConversationClick = (id: string) => {
    navigate(`/chat/${id}`)
  }

  // Section header matching the LeftSidebar's `SectionHeader` style
  // (fontSizeSM + colorTextSecondary). Suppressed only when scoped
  // to a specific project (ProjectDetailPage renders its own list
  // framing). Both sidebar variants — default ("all conversations")
  // and the unfiled variant the chat module wraps for the desktop
  // sidebar — get the header.
  // Match LeftSidebar's `SectionHeader` exactly: no wrapping
  // padding, `px-3 pb-0.5` on the Text, same font size and color.
  // The parent slot wrapper already provides the vertical rhythm.
  const header =
    typeof projectIdFilter === 'string' ? null : (
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

  if (!loading && conversations.length === 0) {
    return (
      <div className="flex flex-col h-full">
        {header}
        <div className="px-2 py-4">
          <Empty
            image={<MessageOutlined className="text-4xl text-gray-400" />}
            description={
              <Text type="secondary" className="text-xs">
                {projectIdFilter === null
                  ? 'No unfiled conversations'
                  : projectIdFilter
                    ? 'No conversations in this project'
                    : 'No conversations yet'}
              </Text>
            }
            styles={{ image: { height: 40 } }}
          />
        </div>
      </div>
    )
  }

  const confirmDelete = (id: string, title: string) => {
    Modal.confirm({
      title: 'Delete conversation?',
      content: `"${title}" will be permanently deleted.`,
      okText: 'Delete',
      cancelText: 'Cancel',
      okButtonProps: { danger: true },
      onOk: async () => {
        setDeletingId(id)
        try {
          await Stores.ChatHistory.__state.deleteConversation(id)
          if (projectIdFilter !== undefined) {
            setLocalConvs(prev => prev.filter(c => c.id !== id))
          }
        } finally {
          setDeletingId(null)
        }
      },
    })
  }

  return (
    <div className="flex flex-col h-full min-h-0">
      {header}
      <DivScrollY className="flex-col flex-1 min-h-0">
        {conversations.map((conversation: ConversationResponse) => {
          const isHovered = hoveredId === conversation.id
          const isDeleting = deletingId === conversation.id
          // Menu items now; will grow (rename, pin, archive, etc.).
          const menuItems = [
            {
              key: 'delete',
              danger: true,
              icon: <DeleteOutlined />,
              label: 'Delete',
              onClick: () =>
                confirmDelete(
                  conversation.id,
                  conversation.title || 'Untitled Conversation',
                ),
            },
          ]

          return (
            // Match LeftSidebar's `SidebarItem`: same `px-3 py-1
            // mx-2 rounded-md` shape AND the same hover colors
            // (`colorPrimaryHover` bg + `colorTextLightSolid` text)
            // so the recent-chats list reads as a peer of the
            // Navigation / Tools rows.
            <div
              key={conversation.id}
              className="group relative px-3 py-1 mx-2 cursor-pointer rounded-md"
              style={{
                backgroundColor: isHovered
                  ? token.colorPrimaryHover
                  : 'transparent',
                color: isHovered
                  ? token.colorTextLightSolid
                  : token.colorTextBase,
                transition: 'background-color 150ms, color 150ms',
              }}
              onClick={() => handleConversationClick(conversation.id)}
              onMouseEnter={() => setHoveredId(conversation.id)}
              onMouseLeave={() => setHoveredId(null)}
            >
              <Text
                className="text-sm truncate block"
                // Inherit the row's color so hover swaps the title
                // to colorTextLightSolid like SidebarItem.
                style={{ color: 'inherit' }}
                title={conversation.title || 'Untitled Conversation'}
              >
                {conversation.title || 'Untitled Conversation'}
              </Text>

              {/*
                More-options trigger. Absolutely positioned so it
                overlays the title's right edge on hover instead of
                stealing horizontal space (title would otherwise
                truncate sooner). When idle, opacity:0 + the row's
                background showing through hides it; on hover the
                row's primary-hover bg makes the button area
                visually distinct without a separate backdrop.
              */}
              <div
                className="absolute right-2 top-1/2 -translate-y-1/2"
                style={{ width: 24, height: 24 }}
                onClick={e => e.stopPropagation()}
              >
                <Dropdown
                  menu={{ items: menuItems }}
                  trigger={['click']}
                  placement="bottomRight"
                >
                  <Button
                    type="text"
                    size="small"
                    icon={<MoreOutlined />}
                    loading={isDeleting}
                    style={{
                      width: 24,
                      height: 24,
                      padding: 0,
                      // Solid chip on top of the primary-hover row
                      // so the 3-dot reads as clearly clickable
                      // instead of blending into the hover color.
                      backgroundColor: token.colorBgContainer,
                      color: token.colorTextBase,
                      border: `1px solid ${token.colorBorderSecondary}`,
                      opacity: isHovered || isDeleting ? 1 : 0,
                      transition: 'opacity 120ms ease-out',
                    }}
                    aria-label="Conversation options"
                  />
                </Dropdown>
              </div>
            </div>
          )
        })}
      </DivScrollY>
    </div>
  )
}
