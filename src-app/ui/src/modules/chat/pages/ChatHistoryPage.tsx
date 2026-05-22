import { useRef } from 'react'
import { Button, Typography } from 'antd'
import { MessageOutlined, PlusOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { ConversationList } from '@/modules/chat/components/ConversationList'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { DivScrollY } from '@/components/common/DivScrollY'

const { Title, Text } = Typography

/**
 * ChatHistoryPage
 * Displays the full chat history with search, pagination, and bulk operations
 */
export default function ChatHistoryPage() {
  const navigate = useNavigate()
  const searchBoxContainerRef = useRef<HTMLDivElement>(null)

  // Chat history store for empty state detection
  const { conversations, loading } = Stores.ChatHistory

  return (
    <div className="h-full w-full flex flex-col overflow-y-hidden">
      {/* Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Typography.Title level={4} className="!m-0 !leading-tight">
            Chat History
          </Typography.Title>
        </div>
      </HeaderBarContainer>

      {/* Content */}
      <div className="flex-1 flex flex-col overflow-hidden items-center">
        {/* Show ConversationList if there are conversations or loading */}
        {(conversations.length > 0 || loading) && (
          <div className="flex flex-1 flex-col w-full justify-center overflow-hidden">
            {/* Search box — always visible above the scrollable list */}
            <div className="w-full max-w-4xl self-center px-3 pt-3">
              <div ref={searchBoxContainerRef} />
            </div>
            <DivScrollY className="h-full flex flex-col">
              <ConversationList
                getSearchBoxContainer={() => searchBoxContainerRef.current}
              />
            </DivScrollY>
          </div>
        )}

        {/* Empty State */}
        {!loading && conversations.length === 0 && (
          <div className="text-center py-12 m-auto">
            <MessageOutlined className="text-6xl mb-4" />
            <Title level={3} type="secondary">
              No chat history yet
            </Title>
            <Text type="secondary" className="block mb-4">
              Start your first conversation to see your chat history here
            </Text>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={() => navigate('/chat')}
            >
              Start New Chat
            </Button>
          </div>
        )}
      </div>
    </div>
  )
}
