import { useEffect, useRef, useState } from 'react'
import { Button, Tooltip, Text, Title } from '@/components/ui'
import { MessageSquare, Plus, Search as SearchIcon } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { ConversationList } from '@/modules/chat/components/ConversationList'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'
import { DivScrollY } from '@/components/common/DivScrollY'
import { useElementMinSize } from '@/modules/layouts/app-layout/hooks/useWindowMinSize'

/**
 * ChatHistoryPage
 * Displays the full chat history with search, pagination, and bulk operations.
 *
 * Search-affordance placement is page-width-aware (not viewport-width):
 *   - Wide page (>sm, >640px): the search <Input> is portaled into the
 *     page header on the right.
 *   - Narrow page (≤sm): a single search ICON button sits in the header.
 *     Clicking it toggles a body-rendered search box above the list.
 *     The header button switches to `variant="default"` while the body
 *     search is open so the user knows the affordance is active.
 *
 * The search box itself lives in `ConversationList`; this page just
 * picks the portal target via the `getSearchBoxContainer` callback.
 */
export default function ChatHistoryPage() {
  const navigate = useNavigate()
  const pageRef = useRef<HTMLDivElement>(null)
  const headerSearchRef = useRef<HTMLDivElement>(null)
  const bodySearchRef = useRef<HTMLDivElement>(null)

  // Page-width-aware breakpoint. `sm` = ≤640px page width. When the
  // page is in a narrow layout (sidebar open + medium window, or
  // small window), collapse the header search to an icon button.
  const minSize = useElementMinSize(pageRef)
  const isNarrow = minSize.sm
  const [searchOpenInNarrow, setSearchOpenInNarrow] = useState(false)

  // Chat history store for empty state detection
  const { conversations, loading } = Stores.ChatHistory

  // Refetch on mount. The sidebar's RecentConversationsWidget may have
  // eager-primed the store with an empty list at login (before any
  // conversations existed), leaving `isInitialized=true` and the
  // render below short-circuiting into the empty state — which means
  // `<ConversationList>` never mounts and its own load-on-mount
  // useEffect never fires. Trigger the refetch here so newly-created
  // conversations always appear.
  useEffect(() => {
    Stores.ChatHistory.__state.loadConversations()
  }, [])

  // Closing the body search affordance when the page grows back to
  // wide — keeps the UI tidy after a resize.
  useEffect(() => {
    if (!isNarrow) setSearchOpenInNarrow(false)
  }, [isNarrow])

  // ConversationList portals its searchBox into whatever element this
  // callback returns. null = hide the search (narrow + button-closed).
  const getSearchBoxContainer = () => {
    if (!isNarrow) return headerSearchRef.current
    if (searchOpenInNarrow) return bodySearchRef.current
    return null
  }

  return (
    <div
      ref={pageRef}
      className="h-full w-full flex flex-col overflow-y-hidden"
    >
      {/* Header */}
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between gap-3 w-full">
          <Title
            level={4}
            className="!m-0 !leading-tight truncate"
          >
            Chats
          </Title>

          {/* Wide layout: inline search input portal target. */}
          {!isNarrow && (
            <div
              ref={headerSearchRef}
              className="flex-[0_1_320px] min-w-[200px]"
            />
          )}

          {/* Narrow layout: search ICON button that toggles a body
            * search box. Becomes `variant="default"` when the body search is
            * open so the active state is visible. */}
          {isNarrow && (
            <Tooltip
              content={searchOpenInNarrow ? 'Hide search' : 'Search'}
            >
              <Button
                data-testid="chat-history-search-toggle-btn"
                variant={searchOpenInNarrow ? 'default' : 'ghost'}
                icon={<SearchIcon />}
                onClick={() => setSearchOpenInNarrow(v => !v)}
                aria-label={
                  searchOpenInNarrow ? 'Hide search' : 'Open search'
                }
                aria-pressed={searchOpenInNarrow}
              />
            </Tooltip>
          )}
        </div>
      </HeaderBarContainer>

      {/* Content */}
      <div className="flex-1 flex flex-col overflow-hidden items-center">
        {/* Body search box — always rendered when narrow + opened via
         * header button, so it works even in the empty state. */}
        {isNarrow && searchOpenInNarrow && (
          <div className="w-full max-w-4xl self-center px-3 pt-3">
            <div ref={bodySearchRef} />
          </div>
        )}
        {/* Show ConversationList if there are conversations or loading */}
        {(conversations.length > 0 || loading) && (
          <div className="flex flex-1 flex-col w-full justify-center overflow-hidden">
            <DivScrollY className="h-full flex flex-col">
              <ConversationList
                getSearchBoxContainer={getSearchBoxContainer}
              />
            </DivScrollY>
          </div>
        )}

        {/* Empty State */}
        {!loading && conversations.length === 0 && (
          <div className="text-center py-12 m-auto">
            <MessageSquare className="text-6xl mb-4" />
            <Title level={3}>
              No chat history yet
            </Title>
            <Text type="secondary" className="block mb-4">
              Start your first conversation to see your chat history here
            </Text>
            <Button
              data-testid="chat-history-new-chat-btn"
              variant="default"
              icon={<Plus />}
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
