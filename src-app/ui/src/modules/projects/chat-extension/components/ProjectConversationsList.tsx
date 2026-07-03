import { CircleMinus } from 'lucide-react'
import { Alert, Button, Empty, Tooltip, Text, message, Dialog } from '@/components/ui'
import { useState } from 'react'
import type { ConversationResponse } from '@/api-client/types'
import { Stores } from '@/core/stores'
import { ConversationCard } from '@/modules/chat/components/ConversationCard'

interface ProjectConversationsListProps {
  projectId: string
  selectedIds: Set<string>
  onToggleSelect: (conversationId: string) => void
}

/**
 * Project-scoped conversation list. Renders the same `ConversationCard`
 * as the global ChatHistoryPage so the two surfaces look identical.
 * Adds a per-card "Remove from project" affordance via the card's
 * `trailing` slot — detaches a single conversation from this project.
 *
 * Selection state is lifted to ProjectDetailPage so the parent Card's
 * header can host the bulk-action toolbar.
 */
export function ProjectConversationsList({
  projectId,
  selectedIds,
  onToggleSelect,
}: ProjectConversationsListProps) {
  const {
    conversations,
    conversationsLoading,
    conversationsLoadingMore,
    conversationsHasMore,
    conversationsError,
  } = Stores.ProjectDetail
  const isSelectionMode = selectedIds.size > 0

  if (conversationsLoading && conversations.length === 0) {
    return (
      <div className="flex justify-center py-8">
        <div className="animate-spin rounded-full h-8 w-8 border-b-2"></div>
      </div>
    )
  }

  // Surface a load failure instead of the misleading "no conversations"
  // empty state below.
  if (conversationsError && conversations.length === 0) {
    return (
      <Alert
        data-testid="project-conversations-load-error-alert"
        tone="error"
        title="Failed to load conversations"
        description={conversationsError}
      />
    )
  }

  if (conversations.length === 0) {
    return (
      <Empty data-testid="project-conversations-empty" description="No conversations in this project yet">
        <Text type="secondary">
          Start a new chat here and it will inherit this project's
          instructions + knowledge.
        </Text>
      </Empty>
    )
  }

  const handleDelete = async (id: string) => {
    await Stores.ChatHistory.__state.deleteConversation(id)
  }

  const handleLoadMore = () => {
    void Stores.ProjectDetail.loadMoreConversations(projectId)
  }

  const renderTrailing = (conv: ConversationResponse) => (
    <RemoveFromProjectButton projectId={projectId} conversationId={conv.id} />
  )

  return (
    <div className="flex flex-col gap-3">
      {conversations.map(conversation => (
        <ConversationCard
          key={conversation.id}
          conversation={conversation}
          isSelected={selectedIds.has(conversation.id)}
          isInSelectionMode={isSelectionMode}
          onSelect={onToggleSelect}
          onDelete={handleDelete}
          trailing={renderTrailing}
        />
      ))}
      {conversationsHasMore && (
        <div className="flex justify-center pt-1">
          <Button
            data-testid="project-conversations-load-more-button"
            onClick={handleLoadMore}
            loading={conversationsLoadingMore}
          >
            Load More
          </Button>
        </div>
      )}
    </div>
  )
}

/**
 * Hover-revealed icon button that detaches the conversation from the
 * project. Same visual rhythm as ConversationCard's delete button
 * (text + small + bg-container) so the row stays visually balanced.
 */
function RemoveFromProjectButton({
  projectId,
  conversationId,
}: {
  projectId: string
  conversationId: string
}) {
  const [open, setOpen] = useState(false)
  const [loading, setLoading] = useState(false)

  const handleRemove = async () => {
    setLoading(true)
    try {
      await Stores.Projects.detachConversation(projectId, conversationId)
      message.success('Removed from project')
    } catch (err) {
      message.error(
        err instanceof Error ? err.message : 'Failed to remove from project',
      )
    } finally {
      setLoading(false)
      setOpen(false)
    }
  }

  return (
    <>
      <Dialog
        data-testid="project-conv-remove-dialog"
        open={open}
        onOpenChange={(v) => { if (!v) setOpen(false) }}
        title="Remove from project?"
      >
        <p className="text-muted-foreground">
          The conversation will become unfiled. It is NOT deleted.
        </p>
        <div className="flex justify-end gap-2 mt-6">
          <Button
            data-testid="project-conv-remove-cancel-button"
            onClick={() => setOpen(false)}
            variant="outline"
          >
            Cancel
          </Button>
          <Button
            data-testid="project-conv-remove-confirm-button"
            onClick={handleRemove}
            variant="destructive"
            disabled={loading}
          >
            {loading ? 'Removing...' : 'Remove'}
          </Button>
        </div>
      </Dialog>
      <Tooltip content="Remove from project">
        <Button
          data-testid="project-conv-remove-trigger-button"
          className={`transition-opacity bg-card ${
            open ? 'opacity-100' : 'opacity-0 group-hover:opacity-100 hover-none:opacity-100'
          }`}
          variant="ghost"
          size="default"
          icon={<CircleMinus />}
          aria-label="Remove from project"
          onClick={(e: React.MouseEvent) => {
            e.stopPropagation()
            setOpen(true)
          }}
        />
      </Tooltip>
    </>
  )
}
