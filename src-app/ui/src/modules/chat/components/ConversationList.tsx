import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Input, Card, Button, Typography, Empty, Flex, Popconfirm, App } from 'antd'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { SearchOutlined, DeleteOutlined, CloseCircleOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { ConversationCard } from '@/modules/chat/components/ConversationCard'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'

const { Text } = Typography

interface ConversationListProps {
  /**
   * Function that returns container element for search box (for portal rendering)
   */
  getSearchBoxContainer?: () => HTMLElement | null
}

/**
 * ConversationList Component
 * Displays a searchable, paginated list of conversations with bulk operations
 */
export function ConversationList({ getSearchBoxContainer }: ConversationListProps) {
  const { message } = App.useApp()
  const [, forceRender] = useState({})
  const [localSearchQuery, setLocalSearchQuery] = useState('')
  const canDelete = usePermission(Permissions.ConversationsDelete)

  const {
    conversations,
    filteredConversations,
    searchQuery,
    selectedIds,
    loading,
    loadingMore,
    deleting,
    hasMore,
    total,
    isInitialized,
    error,
  } = Stores.ChatHistory

  // Force a second render when getSearchBoxContainer is provided to ensure container is available
  useEffect(() => {
    if (getSearchBoxContainer) {
      forceRender({})
    }
  }, [getSearchBoxContainer])

  // Show errors
  useEffect(() => {
    if (error) {
      message.error(error)
    }
  }, [error, message])

  // Debounce search query
  useEffect(() => {
    const timeoutId = setTimeout(() => {
      Stores.ChatHistory.__state.setSearchQuery(localSearchQuery)
    }, 500)

    return () => clearTimeout(timeoutId)
  }, [localSearchQuery])

  // Load conversations on mount — always re-fetch, not guarded by
  // `isInitialized`. The sidebar's RecentConversationsWidget calls
  // `loadConversations()` at login and flips `isInitialized=true`
  // with whatever conversations existed at that moment. If
  // conversations are created later (by another tab, an MCP tool,
  // or — in the E2E suite — a test that seeds before navigating
  // here), the dedicated `/chats` page must show them.
  // `loadConversations` already dedupes concurrent calls via its
  // internal `loading/loadingMore` in-flight check, so unconditional
  // refetch is safe.
  useEffect(() => {
    Stores.ChatHistory.__state.loadConversations()
  }, [])

  const handleLoadMore = async () => {
    try {
      await Stores.ChatHistory.__state.loadNextPage()
    } catch (error) {
      console.error('Failed to load more conversations:', error)
    }
  }

  const handleDeleteSelected = async () => {
    try {
      await Stores.ChatHistory.__state.bulkDelete()
      message.success(`${selectedIds.size} conversations deleted successfully`)
    } catch (error) {
      console.error('Failed to delete selected conversations:', error)
    }
  }

  const handleToggleSelection = (id: string) => {
    Stores.ChatHistory.__state.toggleSelection(id)
  }

  const handleDeleteConversation = async (id: string) => {
    await Stores.ChatHistory.__state.deleteConversation(id)
  }

  // Determine which conversations to show
  const visibleConversations = searchQuery ? filteredConversations : conversations
  const isSelectionMode = selectedIds.size > 0

  // Search box component
  const searchBox = (
    <Input
      placeholder="Search conversations..."
      allowClear
      size="middle"
      prefix={<SearchOutlined />}
      onChange={e => setLocalSearchQuery(e.target.value)}
      className="self-center w-full"
      value={localSearchQuery}
    />
  )

  return (
    <>
      {/* Render search box in portal if container provided */}
      {getSearchBoxContainer &&
        (() => {
          const container = getSearchBoxContainer()
          return container ? createPortal(searchBox, container) : null
        })()}

      <div className="w-full h-full flex flex-col gap-3 overflow-y-hidden overflow-x-visible flex-1">
        {/* Search box - render inline if no container provided */}
        {!getSearchBoxContainer && (
          <div className="flex justify-end items-center w-full">{searchBox}</div>
        )}

        {/* Bulk actions bar */}
        {selectedIds.size > 0 && (
          <div className="max-w-4xl w-full self-center px-3 pt-3">
            <Card
              classNames={{
                body: '!p-3',
              }}
            >
              <Flex
                justify="space-between"
                align="center"
                className="flex-wrap gap-2"
              >
                <Text strong>
                  {selectedIds.size} conversation{selectedIds.size > 1 ? 's' : ''} selected
                </Text>
                <Flex className="gap-2">
                  <Button
                    icon={<CloseCircleOutlined />}
                    onClick={() => Stores.ChatHistory.__state.deselectAll()}
                  >
                    Deselect All
                  </Button>
                  <Button onClick={() => Stores.ChatHistory.__state.selectAll()}>
                    Select All
                  </Button>
                  {canDelete && (
                    <Popconfirm
                      title="Delete selected conversations"
                      description={`Are you sure you want to delete ${selectedIds.size} conversation${selectedIds.size > 1 ? 's' : ''}?`}
                      onConfirm={handleDeleteSelected}
                      okText="Delete"
                      cancelText="Cancel"
                      okType="danger"
                      okButtonProps={{ loading: deleting }}
                    >
                      <Button danger icon={<DeleteOutlined />} loading={deleting}>
                        Delete Selected
                      </Button>
                    </Popconfirm>
                  )}
                </Flex>
              </Flex>
            </Card>
          </div>
        )}

        {/* Conversation list */}
        <DivScrollY className="flex-1 w-full flex-col !py-3 overflow-x-visible">
          <div className="gap-2 max-w-4xl w-full self-center overflow-x-visible">
            {visibleConversations.length === 0 && !loading ? (
              <Card className="!mx-3">
                <Empty
                  image={Empty.PRESENTED_IMAGE_SIMPLE}
                  description={
                    searchQuery
                      ? 'No conversations found matching your search'
                      : 'No chat history yet'
                  }
                />
              </Card>
            ) : (
              <DivScrollY className="space-y-3 overflow-x-visible">
                {loading && !isInitialized ? (
                  <div className="flex justify-center py-8">
                    <div className="animate-spin rounded-full h-8 w-8 border-b-2"></div>
                  </div>
                ) : (
                  // Plain div instead of DivScrollY: the outer
                  // DivScrollY already handles scroll. DivScrollY
                  // wraps its children in an internal flex-col
                  // container, so any `gap-*` on it lands on the
                  // OverlayScrollbars wrapper and never reaches the
                  // card siblings — that's why cards had no gap.
                  <div className="flex flex-col gap-3 w-full flex-1 overflow-x-visible">
                    {visibleConversations.map((conversation: ConversationResponse) => (
                      <div key={conversation.id} className="px-3">
                        <ConversationCard
                          conversation={conversation}
                          isSelected={selectedIds.has(conversation.id)}
                          isInSelectionMode={isSelectionMode}
                          onSelect={handleToggleSelection}
                          onDelete={handleDeleteConversation}
                        />
                      </div>
                    ))}

                    {/* Pagination info */}
                    {visibleConversations.length > 0 && (
                      <Card
                        className="text-center !mx-3"
                        classNames={{
                          body: '!p-2 gap-2 flex justify-center items-center flex-wrap',
                        }}
                      >
                        <Text type="secondary">
                          Showing {visibleConversations.length} of {total} conversations
                        </Text>
                        {hasMore && !searchQuery && (
                          <Button onClick={handleLoadMore} loading={loadingMore}>
                            Load More
                          </Button>
                        )}
                      </Card>
                    )}
                  </div>
                )}
              </DivScrollY>
            )}
          </div>
        </DivScrollY>
      </div>
    </>
  )
}
