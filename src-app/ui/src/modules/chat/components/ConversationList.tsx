import { memo, useCallback, useEffect, useRef, useState } from 'react'
import { createPortal } from 'react-dom'
import type { OverlayScrollbarsComponentRef } from 'overlayscrollbars-react'
import { Card, Button, Text, Empty, ErrorState, Flex, Confirm, Input, message } from '@ziee/kit'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { CircleX, Search as SearchIcon, Trash2 } from 'lucide-react'
import { Stores } from '@/core/stores'
import { ConversationCard } from '@/modules/chat/components/ConversationCard'
import { VirtualizedConversationList } from '@/modules/chat/components/VirtualizedConversationList'
import type { ConversationResponse } from '@/api-client/types'
import { DivScrollY } from '@/components/common/DivScrollY'
import { cn } from '@/lib/utils'


interface ConversationListProps {
  /**
   * Function that returns container element for search box (for portal rendering)
   */
  getSearchBoxContainer?: () => HTMLElement | null
}

/**
 * Memoized row card: with stable `onSelect`/`onDelete` callbacks and a stable
 * per-row `conversation` reference, a scroll-driven virtualizer re-render skips
 * re-rendering rows whose selection didn't change (so `dayjs.fromNow` /
 * `usePermission` / `useNavigate` aren't recomputed for every visible row on
 * every scroll frame). ConversationCard itself is left untouched so its other
 * consumers (project page, recent widget) are unaffected (DEC-9).
 */
const MemoConversationCard = memo(ConversationCard)

/**
 * ConversationList Component
 * Displays a searchable, paginated list of conversations with bulk operations
 */
export function ConversationList({ getSearchBoxContainer }: ConversationListProps) {
  const [, forceRender] = useState({})
  const [localSearchQuery, setLocalSearchQuery] = useState('')
  const canDelete = usePermission(Permissions.ConversationsDelete)
  const { nativeScroll } = Stores.AppLayout

  // Row virtualization (chats-page-virtualization ITEM-4): the card list scrolls
  // inside this OverlayScrollbars viewport on desktop. Resolve its root element
  // for the virtualizer and flip `scrollerReady` once the OS instance mounts so
  // the virtualizer re-observes the real viewport (mirrors ConversationPage). On
  // the mobile native-scroll path DivScrollY renders a plain flow div (no OS
  // instance) → `getScrollElement` returns null and the list renders plainly.
  const scrollerRef = useRef<OverlayScrollbarsComponentRef>(null)
  const [scrollerReady, setScrollerReady] = useState(false)
  const getScrollElement = useCallback((): HTMLElement | null => {
    const os = scrollerRef.current?.osInstance()
    return (os?.elements().viewport as HTMLElement | undefined) ?? null
  }, [])

  const {
    conversations,
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

  // Debounce search query
  useEffect(() => {
    const timeoutId = setTimeout(() => {
      Stores.ChatHistory.setSearchQuery(localSearchQuery)
    }, 500)

    return () => clearTimeout(timeoutId)
  }, [localSearchQuery])

  // Load conversations on mount — always re-fetch, not guarded by
  // `isInitialized`. This `/chats` list owns its own `conversations` fetch
  // (the sidebar's RecentConversationsWidget now loads a SEPARATE
  // `recentConversations` cursor, so it no longer seeds this list). If
  // conversations are created later (by another tab, an MCP tool, or — in the
  // E2E suite — a test that seeds before navigating here), the dedicated
  // `/chats` page must show them. `loadConversations` already dedupes
  // concurrent calls via its internal `loading/loadingMore` in-flight check,
  // so an unconditional refetch is safe.
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

  // Stable across renders so the memoized row card (MemoConversationCard) can
  // skip re-rendering unchanged rows on every scroll-driven virtualizer update.
  const handleToggleSelection = useCallback((id: string) => {
    Stores.ChatHistory.toggleSelection(id)
  }, [])

  const handleDeleteConversation = useCallback(async (id: string) => {
    await Stores.ChatHistory.deleteConversation(id)
  }, [])

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

        {/* Sort control moved to the page header (ChatHistorySortControl). */}

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
        <DivScrollY
          nativeFlow
          className="flex-1 w-full flex-col !py-3 overflow-x-visible"
          ref={scrollerRef}
          events={{ initialized: () => setScrollerReady(true) }}
        >
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
                  // Row-virtualized on desktop (inner OverlayScrollbars viewport);
                  // plain render on the mobile native-scroll path. Only the visible
                  // window of ConversationCards is mounted regardless of how many
                  // pages have been loaded (chats-page-virtualization ITEM-3/5). The
                  // "Showing N of M" + Load-More block is a NON-virtualized footer
                  // sibling below the rows so its testids + aria-live status stay
                  // exactly as before.
                  <VirtualizedConversationList
                    conversations={visibleConversations}
                    virtualize={!nativeScroll}
                    getScrollElement={getScrollElement}
                    scrollerReady={scrollerReady}
                    renderCard={(conversation: ConversationResponse) => (
                      <MemoConversationCard
                        conversation={conversation}
                        isSelected={selectedIds.has(conversation.id)}
                        isInSelectionMode={isSelectionMode}
                        onSelect={handleToggleSelection}
                        onDelete={handleDeleteConversation}
                      />
                    )}
                    footer={
                      visibleConversations.length > 0 ? (
                        <div
                          data-testid="chat-history-pagination-card"
                          className="text-center px-3 py-2 flex flex-col items-center gap-2"
                        >
                          <Text type="secondary" aria-live="polite" role="status">
                            Showing {visibleConversations.length} of {total}{' '}
                            {total === 1 ? 'conversation' : 'conversations'}
                          </Text>
                          {hasMore && (
                            <Button data-testid="chat-history-load-more-btn" onClick={handleLoadMore} loading={loadingMore}>
                              Load More
                            </Button>
                          )}
                        </div>
                      ) : null
                    }
                  />
                )}
              </div>
            )}
          </div>
        </DivScrollY>
      </div>
    </>
  )
}
