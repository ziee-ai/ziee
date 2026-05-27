import { useEffect, useRef, useState } from 'react'
import { Typography, Button, Popconfirm, Empty, Spin, Divider } from 'antd'
import { MessageOutlined, DeleteOutlined, UnorderedListOutlined } from '@ant-design/icons'
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

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setDeletingId(id)
    try {
      await Stores.ChatHistory.__state.deleteConversation(id)
      // In filtered mode, also drop from local state immediately.
      if (projectIdFilter !== undefined) {
        setLocalConvs(prev => prev.filter(c => c.id !== id))
      }
    } catch (_error) {
      setDeletingId(null)
    }
  }

  if (loading && !isInitialized) {
    return (
      <div className="flex justify-center items-center py-8">
        <Spin />
      </div>
    )
  }

  if (!loading && conversations.length === 0) {
    return (
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
    )
  }

  return (
    <DivScrollY className="flex-col h-full">
      {conversations.map((conversation: ConversationResponse) => (
        <div
          key={conversation.id}
          className="group relative px-3 py-2  cursor-pointer rounded transition-colors"
          onClick={() => handleConversationClick(conversation.id)}
          onMouseEnter={() => setHoveredId(conversation.id)}
          onMouseLeave={() => setHoveredId(null)}
        >
          <div className="flex items-center justify-between gap-2">
            <div className="flex-1 min-w-0">
              <Text
                className="text-sm truncate block"
                title={conversation.title || 'Untitled Conversation'}
              >
                {conversation.title || 'Untitled Conversation'}
              </Text>
            </div>

            {/* Delete button (visible on hover) */}
            {hoveredId === conversation.id && (
              <div onClick={e => e.stopPropagation()}>
                <Popconfirm
                  title="Delete conversation?"
                  description="This will permanently delete the conversation."
                  onConfirm={e => handleDelete(conversation.id, e as any)}
                  okText="Delete"
                  cancelText="Cancel"
                  okButtonProps={{ danger: true }}
                >
                  <Button
                    type="text"
                    danger
                    size="small"
                    icon={<DeleteOutlined />}
                    loading={deletingId === conversation.id}
                    className="opacity-100"
                  />
                </Popconfirm>
              </div>
            )}
          </div>

          {/* Message count */}
          <div className="mt-1">
            <Text type="secondary" className="text-xs">
              {conversation.message_count}{' '}
              {conversation.message_count === 1 ? 'message' : 'messages'}
            </Text>
          </div>
        </div>
      ))}

      <Divider className="!my-1" />
      <div className="px-2 pb-2">
        <Button
          type="text"
          icon={<UnorderedListOutlined />}
          block
          onClick={() => navigate('/chats')}
        >
          All conversations
        </Button>
      </div>
    </DivScrollY>
  )
}
