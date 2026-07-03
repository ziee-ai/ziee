import { type ReactNode, useState } from 'react'
import { Button, Card, Checkbox, Confirm, Separator, Text, Tooltip } from '@/components/ui'
import { message } from '@/components/ui'
import { usePermission } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { Trash2 } from 'lucide-react'
import { useNavigate } from 'react-router-dom'
import dayjs from 'dayjs'
import relativeTime from 'dayjs/plugin/relativeTime'
import type { ConversationResponse } from '@/api-client/types'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'

dayjs.extend(relativeTime)

interface ConversationCardProps {
  conversation: ConversationResponse
  onDelete: (conversationId: string) => Promise<void>
  isSelected?: boolean
  onSelect?: (conversationId: string) => void
  isInSelectionMode?: boolean
  /**
   * Extra controls rendered in the per-card bottom-right action row,
   * left of the Delete button + checkbox. Like Delete, hidden in
   * selection mode (bulk actions take over the row). Use for
   * caller-specific per-row affordances (e.g., "Remove from project"
   * on the project page).
   */
  trailing?: (conversation: ConversationResponse) => ReactNode
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
  trailing,
}: ConversationCardProps) {
  const navigate = useNavigate()
  const [popconfirmOpen, setPopconfirmOpen] = useState(false)
  const canDelete = usePermission(Permissions.ConversationsDelete)
  // Lazy-render the trailing area only after first hover so extensions
  // that need a network round-trip (e.g., project membership lookup)
  // don't fire N requests per page load. Sticky once true — the
  // user has already paid for the lookup, no point hiding again.
  const [hoveredOnce, setHoveredOnce] = useState(false)

  const handleCardClick = () => {
    if (isInSelectionMode && onSelect) {
      // In selection mode, toggle selection instead of navigating
      onSelect(conversation.id)
    } else {
      // Per-conversation URL resolution: chat extensions can
      // override the default `/chat/{id}` link via the
      // `conversationHref` hook. First non-undefined wins.
      const href =
        chatExtensionRegistry.conversationHref(conversation) ??
        `/chat/${conversation.id}`
      navigate(href)
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

  const handleSelectChange = () => {
    if (onSelect) {
      onSelect(conversation.id)
    }
  }

  // Trailing content: prop wins (caller-supplied, project page's
  // per-row Remove etc.). Otherwise consult the extension registry
  // so any chat extension can inject decorations. Either way, the
  // trailing only mounts after first hover (see `hoveredOnce`).
  const renderedTrailing = trailing
    ? trailing(conversation)
    : chatExtensionRegistry.renderConversationCardTrailing(conversation)

  return (
    <Card
      data-testid={`chat-conversation-card-${conversation.id}`}
      key={conversation.id}
      role="button"
      tabIndex={0}
      aria-label={conversation.title || 'Untitled Conversation'}
      onClick={handleCardClick}
      onKeyDown={e => {
        if (e.key === 'Enter' || e.key === ' ') {
          e.preventDefault()
          handleCardClick()
        }
      }}
      onMouseEnter={() => {
        if (!hoveredOnce) setHoveredOnce(true)
      }}
      className={`cursor-pointer relative group hover:!shadow-md transition-shadow focus-visible:outline focus-visible:outline-2${isSelected ? ' border-primary' : ''}`}
      hoverable
    >
      <div className="flex flex-col gap-2 pb-6">
        {/* Title and metadata */}
        <div className="flex items-start justify-between gap-2">
          <Text strong className="text-base flex-1 min-w-0" ellipsis>
            {conversation.title || 'Untitled Conversation'}
          </Text>
          <div className="flex items-center gap-x-1 flex-shrink-0">
            {conversation.message_count > 0 && (
              <>
                <Text type="secondary" className="text-xs">
                  {conversation.message_count} message{conversation.message_count !== 1 ? 's' : ''}
                </Text>
                <Separator orientation="vertical" className="!mx-1" />
              </>
            )}
            <Text type="secondary" className="whitespace-nowrap text-xs">
              {dayjs(conversation.updated_at).fromNow()}
            </Text>
          </div>
        </div>
      </div>

      {/* Per-card actions row — bottom right. Caller-supplied
          `trailing` + Delete + selection checkbox share one row so
          they don't compete for space on hover. Trailing + Delete
          are hover-revealed; the checkbox stays visible
          (opacity-100) whenever the row is selected. */}
      <div
        className="absolute bottom-2 right-2 flex items-center gap-2 z-10"
        onClick={e => e.stopPropagation()}
      >
        {/* Trailing — caller prop wins, else extension registry.
            Lazy-mounted on first hover (so a slow lookup inside the
            trailing doesn't fire on render). Hidden in selection
            mode (bulk toolbar takes over).
            VISIBILITY is each trailing element's own responsibility —
            it must apply `opacity-0 group-hover:opacity-100` to itself
            (same pattern as the Delete button), AND pin itself
            `opacity-100` while any popover/popconfirm it owns is
            open, so the user can move the mouse off the card to
            interact with the overlay without the anchor disappearing. */}
        {hoveredOnce && !isInSelectionMode && renderedTrailing}

        {/* Delete button — hidden in selection mode (bulk-delete in
            the toolbar replaces per-row deletes). */}
        {canDelete && !isInSelectionMode && (
          <Confirm
            data-testid={`chat-conversation-delete-confirm-${conversation.id}`}
            title="Delete conversation?"
            description="This will permanently delete the conversation and all its messages."
            onConfirm={async () => {
              await handleDeleteConversation()
              setPopconfirmOpen(false)
            }}
            onCancel={() => setPopconfirmOpen(false)}
            okText="Delete"
            cancelText="Cancel"
          >
            <Button
              data-testid={`chat-conversation-delete-btn-${conversation.id}`}
              tooltip="Delete conversation"
              className={`transition-opacity bg-card ${
                popconfirmOpen
                  ? 'opacity-100'
                  : 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100 group-focus-within:opacity-100'
              }`}
              variant="outline"
              size="default"
              icon={<Trash2 />}
              onClick={(e: React.MouseEvent) => {
                e.stopPropagation()
                setPopconfirmOpen(true)
              }}
            />
          </Confirm>
        )}

        {/* Selection checkbox — visible on hover OR when selected. */}
        {onSelect && (
          <div
            className={`transition-opacity ${
              isSelected
                ? 'opacity-100'
                : 'opacity-0 group-hover:opacity-100 group-focus-within:opacity-100'
            }`}
            onClick={e => e.stopPropagation()}
          >
            <Tooltip title={isSelected ? 'Deselect conversation' : 'Select conversation'}>
              <Checkbox
                data-testid={`chat-conversation-select-${conversation.id}`}
                checked={isSelected}
                onChange={handleSelectChange}
                aria-label={isSelected ? 'Deselect conversation' : 'Select conversation'}
              />
            </Tooltip>
          </div>
        )}
      </div>
    </Card>
  )
}
