import { useEffect, useState } from 'react'
import { createPortal } from 'react-dom'
import { Card, Button, Select, Text, Empty, ErrorState, Flex, Confirm, Input, message } from '@/components/ui'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { CircleX, Search as SearchIcon, Trash2 } from 'lucide-react'
import { Stores } from '@/core/stores'
import { ConversationCard } from '@/modules/chat/components/ConversationCard'
import type { ConversationResponse } from '@/api-client/types'
import type { ConversationSort } from '@/modules/chat/stores/ChatHistory.store'
import { DivScrollY } from '@/components/common/DivScrollY'
import { cn } from '@/lib/utils'

const SORT_OPTIONS: { value: ConversationSort; label: string }[] = [
  { value: 'recent', label: 'Most recent' },
  { value: 'oldest', label: 'Oldest first' },
  { value: 'alpha', label: 'Title A–Z' },
  { value: 'most_messages', label: 'Most messages' },
]

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
  const [, forceRender] = useState({})
  const [localSearchQuery, setLocalSearchQuery] = useState('')
  const canDelete = usePermission(Permissions.ConversationsDelete)
  const { nativeScroll } = Stores.AppLayout

  const {
    conversations,
    searchQuery,
    sort,
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

  // Debounce search query
  useEffect(() => {
    const timeoutId = setTimeout(() => {
      Stores.ChatHistory.setSearchQuery(localSearchQuery)
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
    Stores.ChatHistory.loadConversations()
  }, [])

  const handleLoadMore = async () => {
    try {
      await Stores.ChatHistory.loadNextPage()
    } catch (error) {
      console.error('Failed to load more conversations:', error)
    }
  }

  const handleDeleteSelected = async () => {
    try {
      await Stores.ChatHistory.bulkDelete()
      message.success(`${selectedIds.size} conversations deleted successfully`)
    } catch (error) {
      console.error('Failed to delete selected conversations:', error)
    }
  }

  const handleToggleSelection = (id: string) => {
    Stores.ChatHistory.toggleSelection(id)
  }

  const handleDeleteConversation = async (id: string) => {
    await Stores.ChatHistory.deleteConversation(id)
  }

  // The list is the server-filtered/sorted result set directly.
  const visibleConversations = conversations
  const isSelectionMode = selectedIds.size > 0

  // Search box component
  const searchBox = (
    <Input
      data-testid="chat-conversation-search-input"
      placeholder="Search conversations..."
      allowClear
      prefix={<SearchIcon />}
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

      <div className={cn('w-full flex flex-col gap-3 overflow-x-visible flex-1', nativeScroll ? '' : 'h-full overflow-y-hidden')}>
        {/* Search box - render inline if no container provided */}
        {!getSearchBoxContainer && (
          <div className="flex justify-end items-center w-full">{searchBox}</div>
        )}

        {/* Sort control (ITEM-6). Body-level toolbar so it's visible in both the
            wide (header-portaled search) and narrow layouts. Hidden until there
            is a list to sort. */}
        {(visibleConversations.length > 0 || loading) && (
          <div className="max-w-4xl w-full self-center px-3 flex justify-end">
            <Select
              data-testid="chat-history-sort-select"
              aria-label="Sort conversations"
              value={sort}
              onChange={value =>
                Stores.ChatHistory.setSort(value as ConversationSort)
              }
              options={SORT_OPTIONS}
              className="w-40"
            />
          </div>
        )}

        {/* Bulk actions bar */}
        {selectedIds.size > 0 && (
          <div className="max-w-4xl w-full self-center px-3 pt-3">
            <Card data-testid="chat-bulk-actions-card" className="[&_[data-part='body']]:!p-3">
              <Flex
                justify="between"
                align="center"
                className="flex-wrap gap-2"
              >
                <Text strong>
                  {selectedIds.size} conversation{selectedIds.size > 1 ? 's' : ''} selected
                </Text>
                <Flex className="gap-2">
                  <Button
                    data-testid="chat-bulk-deselect-btn"
                    icon={<CircleX />}
                    onClick={() => Stores.ChatHistory.deselectAll()}
                  >
                    Deselect All
                  </Button>
                  <Button
                    data-testid="chat-bulk-select-all-btn"
                    onClick={() => Stores.ChatHistory.selectAll()}
                  >
                    Select All
                  </Button>
                  {canDelete && (
                    <Confirm
                      data-testid="chat-bulk-delete-confirm"
                      title="Delete selected conversations"
                      description={`Are you sure you want to delete ${selectedIds.size} conversation${selectedIds.size > 1 ? 's' : ''}?`}
                      onConfirm={handleDeleteSelected}
                      okText="OK"
                      cancelText="Cancel"
                      okButtonProps={{ danger: true, disabled: deleting }}
                    >
                      <Button data-testid="chat-bulk-delete-btn" variant="ghost" icon={<Trash2 />} loading={deleting}>
                        Delete Selected
                      </Button>
                    </Confirm>
                  )}
                </Flex>
              </Flex>
            </Card>
          </div>
        )}

        {/* Conversation list */}
        <DivScrollY nativeFlow className="flex-1 w-full flex-col !py-3 overflow-x-visible">
          <div className="gap-2 max-w-4xl w-full self-center overflow-x-visible">
            {visibleConversations.length === 0 && !loading ? (
              error ? (
                <div className="px-3">
                  <ErrorState
                    resource="chat history"
                    description="Your chat history couldn't be loaded. Check your connection and try again."
                    details={error}
                    onRetry={() => Stores.ChatHistory.loadConversations()}
                    data-testid="chat-history-error"
                  />
                </div>
              ) : (
                <Card data-testid="chat-history-empty-card" className="!mx-3">
                  <Empty
                    data-testid="chat-history-empty"
                    description={
                      searchQuery
                        ? 'No conversations found matching your search'
                        : 'No chat history yet'
                    }
                  />
                </Card>
              )
            ) : (
              <div className="space-y-3 overflow-x-visible">
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

                    {/* Pagination info — plain text (no card). */}
                    {visibleConversations.length > 0 && (
                      <div
                        data-testid="chat-history-pagination-card"
                        className="text-center px-3 py-2 flex flex-col items-center gap-2"
                      >
                        <Text type="secondary" aria-live="polite" role="status">
                          Showing {visibleConversations.length} of {total} conversations
                        </Text>
                        {hasMore && (
                          <Button data-testid="chat-history-load-more-btn" onClick={handleLoadMore} loading={loadingMore}>
                            Load More
                          </Button>
                        )}
                      </div>
                    )}
                  </div>
                )}
              </div>
            )}
          </div>
        </DivScrollY>
      </div>
    </>
  )
}
