import { useState } from 'react'
import { App, Button, Card, Checkbox, Divider, Popconfirm, theme, Typography } from 'antd'
import { DeleteOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import type { ConversationResponse } from '@/api-client/types'

dayjs.extend(relativeTime)

const { Text } = Typography

interface ConversationCardProps {
  conversation: ConversationResponse
  onDelete: (conversationId: string) => Promise<void>
  isSelected?: boolean
  onSelect?: (conversationId: string) => void
  isInSelectionMode?: boolean
}

/**
 * ConversationCard Component
 * Displays a single conversation with hover effects, selection, and delete functionality
 * Matches reference code design with compact layout
 */
export function ConversationCard({
  conversation,
  onDelete,
  isSelected = false,
  onSelect,
  isInSelectionMode = false,
}: ConversationCardProps) {
  const { message } = App.useApp()
  const navigate = useNavigate()
  const { token } = theme.useToken()
  const [popconfirmOpen, setPopconfirmOpen] = useState(false)

  const handleCardClick = () => {
    if (isInSelectionMode && onSelect) {
      // In selection mode, toggle selection instead of navigating
      onSelect(conversation.id)
    } else {
      // Normal mode, navigate to conversation
      navigate(`/chat/${conversation.id}`)
    }
  }

  const handleDeleteConversation = async () => {
    try {
      await onDelete(conversation.id)
      message.success('Conversation deleted')
    } catch (error) {
      console.error('Failed to delete conversation:', error)
    }
  }

  const handleSelectChange = (e: any) => {
    e.domEvent?.stopPropagation()
    if (onSelect) {
      onSelect(conversation.id)
    }
  }

  return (
    <Card
      key={conversation.id}
      onClick={handleCardClick}
      className="cursor-pointer relative group hover:!shadow-md transition-shadow"
      classNames={{
        body: '!px-3 !py-2',
      }}
      hoverable
      style={{
        borderColor: isSelected ? token.colorPrimary : undefined,
      }}
    >
      <div className="flex flex-col gap-2 pb-6">
        {/* Title and metadata */}
        <div className="flex items-start justify-between gap-2">
          <Text strong className="text-base flex-1 min-w-0" ellipsis={{ tooltip: true }}>
            {conversation.title || 'Untitled Conversation'}
          </Text>
          <div className="flex items-center gap-x-1 flex-shrink-0">
            {conversation.message_count > 0 && (
              <>
                <Text type="secondary" className="text-xs">
                  {conversation.message_count} message{conversation.message_count !== 1 ? 's' : ''}
                </Text>
                <Divider type="vertical" className="!mx-1" />
              </>
            )}
            <Text type="secondary" className="whitespace-nowrap text-xs">
              {dayjs(conversation.updated_at).fromNow()}
            </Text>
          </div>
        </div>
      </div>

      {/* Selection checkbox - positioned in bottom right */}
      {onSelect && (
        <div
          className={`absolute bottom-2 right-2 z-10 transition-opacity ${
            isSelected ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
          }`}
        >
          <Checkbox
            checked={isSelected}
            onChange={handleSelectChange}
            onClick={e => e.stopPropagation()}
          />
        </div>
      )}

      {/* Delete button - positioned in top right */}
      {!isInSelectionMode && (
        <Popconfirm
          title="Delete conversation?"
          description="This will permanently delete the conversation and all its messages."
          open={popconfirmOpen}
          onConfirm={async () => {
            await handleDeleteConversation()
            setPopconfirmOpen(false)
          }}
          onCancel={() => setPopconfirmOpen(false)}
          okText="Yes"
          cancelText="No"
          okButtonProps={{ loading: false }}
        >
          <Button
            className={`!absolute top-2 right-2 transition-opacity ${
              popconfirmOpen ? 'opacity-100' : 'opacity-0 group-hover:opacity-100'
            }`}
            type="text"
            size="small"
            icon={<DeleteOutlined />}
            style={{ backgroundColor: token.colorBgContainer }}
            onClick={(e: React.MouseEvent) => {
              e.stopPropagation()
              setPopconfirmOpen(true)
            }}
          />
        </Popconfirm>
      )}
    </Card>
  )
}
