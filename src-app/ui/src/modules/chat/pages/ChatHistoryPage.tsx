import { useRef, useState } from 'react'
import { Button, Typography } from 'antd'
import { MessageOutlined, PlusOutlined, SearchOutlined } from '@ant-design/icons'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { ConversationList } from '../components/ConversationList'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { useMainContentMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'
import { DivScrollY } from '@/components/common/DivScrollY'

const { Title, Text } = Typography

/**
 * ChatHistoryPage
 * Displays the full chat history with search, pagination, and bulk operations
 */
export default function ChatHistoryPage() {
  const navigate = useNavigate()
  const searchBoxContainerRef = useRef<HTMLDivElement>(null)
  const pageMinSize = useMainContentMinSize()
  const [isSearchBoxVisible, setIsSearchBoxVisible] = useState(false)

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
          <div className="h-full flex items-center justify-between">
            {pageMinSize.xs ? (
              <Button
                type={isSearchBoxVisible ? 'primary' : 'text'}
                icon={<SearchOutlined />}
                style={{ fontSize: '18px' }}
                onClick={() => setIsSearchBoxVisible(!isSearchBoxVisible)}
              />
            ) : (
              <div ref={searchBoxContainerRef} />
            )}
          </div>
        </div>
      </HeaderBarContainer>

      {/* Content */}
      <div className="flex-1 flex flex-col overflow-hidden items-center">
        {/* Mobile search box */}
        {pageMinSize.xs && isSearchBoxVisible && (
          <div className="w-full max-w-96 px-3 pt-3">
            <div ref={searchBoxContainerRef} />
          </div>
        )}

        {/* Show ConversationList if there are conversations or loading */}
        {(conversations.length > 0 || loading) && (
          <div className="flex flex-1 flex-col w-full justify-center overflow-hidden">
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
