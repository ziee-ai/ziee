import { Button, Space, Text } from '@ziee/kit'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { useMessageContext } from '@/modules/chat/core/MessageContext'
import { useChatPaneOrNull } from '@/modules/chat/core/pane/ChatPaneContext'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Core component rendered via MessageContext in ChatMessage.
 * Reads the current message from MessageContext (provided by ChatMessage).
 *
 * Shows a compact < X/N > branch navigator directly below a message bubble
 * when multiple branches diverge at that message.
 *
 * - On the parent branch: rendered below the message that is the fork origin.
 * - On a child branch: rendered below the first new message on that branch.
 *
 * Clicking prev/next activates the target branch, reloading messages.
 */
export function BranchNavigator() {
  const msg = useMessageContext()
  // Bind to THIS pane's store (ITEM-38): activateBranch would otherwise route to
  // the FOCUSED pane, corrupting the other pane's window on same-conversation
  // splits. Captured once so it can't drift across the await.
  const chat = (useChatPaneOrNull()?.store ?? Chat) as typeof Chat
  const { forkPoints, conversation } = chat

  if (!msg || !conversation) return null

  const branchIds = forkPoints.get(msg.id)
  if (!branchIds || branchIds.length <= 1) return null

  const activeBranchId = conversation.active_branch_id ?? ''
  const currentIndex = branchIds.indexOf(activeBranchId)
  const displayIndex = currentIndex === -1 ? 0 : currentIndex
  const total = branchIds.length

  const goTo = async (index: number) => {
    if (index < 0 || index >= total) return
    const branchId = branchIds[index]
    if (!branchId || branchId === activeBranchId) return
    await chat.activateBranch(conversation.id, branchId)
  }

  return (
    <Space size={2} data-testid="branch-navigator">
      <Button
        data-testid="chat-branch-prev-btn"
        variant="ghost"
        size="default"
        aria-label="Previous branch"
        icon={<ChevronLeft />}
        disabled={displayIndex === 0}
        onClick={() => goTo(displayIndex - 1)}
      />
      <Text type="secondary" className="text-xs select-none">
        {displayIndex + 1} / {total}
      </Text>
      <Button
        data-testid="chat-branch-next-btn"
        variant="ghost"
        size="default"
        aria-label="Next branch"
        icon={<ChevronRight />}
        disabled={displayIndex === total - 1}
        onClick={() => goTo(displayIndex + 1)}
      />
    </Space>
  )
}
