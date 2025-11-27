import { useEffect, useState } from 'react'
import { Typography, Button, Popconfirm, Empty, Spin } from 'antd'
import { MessageOutlined, DeleteOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import type { ConversationResponse } from '@/api-client/types'

const { Text } = Typography

/**
 * RecentConversationsWidget
 * Displays recent 20 conversations in the sidebar
 */
export function RecentConversationsWidget() {
  const navigate = useNavigate()
  const { recentConversations, loading, isInitialized } = Stores.ChatHistory
  const [deletingId, setDeletingId] = useState<string | null>(null)
  const [hoveredId, setHoveredId] = useState<string | null>(null)

  // Load conversations on mount
  useEffect(() => {
    if (!isInitialized) {
      Stores.ChatHistory.__state.loadConversations()
    }
  }, [isInitialized])

  const handleConversationClick = (id: string) => {
    navigate(`/chat/${id}`)
  }

  const handleDelete = async (id: string, e: React.MouseEvent) => {
    e.stopPropagation()
    setDeletingId(id)
    try {
      await Stores.ChatHistory.__state.deleteConversation(id)
    } catch (error) {
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

  if (!loading && recentConversations.length === 0) {
    return (
      <div className="px-2 py-4">
        <Empty
          image={<MessageOutlined className="text-4xl text-gray-400" />}
          description={
            <Text type="secondary" className="text-xs">
              No conversations yet
            </Text>
          }
          imageStyle={{ height: 40 }}
        />
      </div>
    )
  }

  return (
    <div className="flex flex-col">
      {recentConversations.map((conversation: ConversationResponse) => (
        <div
          key={conversation.id}
          className="group relative px-3 py-2 hover:bg-gray-100 dark:hover:bg-gray-800 cursor-pointer rounded transition-colors"
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
    </div>
  )
}
