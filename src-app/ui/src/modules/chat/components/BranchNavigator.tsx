import { Button, Space, Text } from '@/components/ui'
import { LeftOutlined, RightOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { useMessageContext } from '@/modules/chat/core/MessageContext'

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
  const { forkPoints, conversation } = Stores.Chat

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
    await Stores.Chat.__state.activateBranch(conversation.id, branchId)
  }

  return (
    <Space size={2} data-testid="branch-navigator">
      <Button
        variant="ghost"
        size="sm"
        icon={<LeftOutlined />}
        disabled={displayIndex === 0}
        onClick={() => goTo(displayIndex - 1)}
      />
      <Text type="secondary" className="text-xs select-none">
        {displayIndex + 1} / {total}
      </Text>
      <Button
        variant="ghost"
        size="sm"
        icon={<RightOutlined />}
        disabled={displayIndex === total - 1}
        onClick={() => goTo(displayIndex + 1)}
      />
    </Space>
  )
}
