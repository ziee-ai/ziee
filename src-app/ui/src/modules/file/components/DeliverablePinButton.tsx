import { Pin, PinOff } from 'lucide-react'
import { Button, message } from '@ziee/kit'
import { Stores } from '@/core/stores'
import { Permissions, type File as FileEntity } from '@/api-client/types'
import { usePermission } from '@/core/permissions'

/**
 * Pin/unpin a file as a deliverable of the ACTIVE conversation. Renders nothing
 * outside a conversation (e.g. the standalone file-preview drawer). Reads the
 * deliverables map reactively so the icon flips live on `sync:deliverable`.
 */
export function DeliverablePinButton({ file }: { file: FileEntity }) {
  // Pin/unpin mutates the conversation's deliverables (`conversations::edit`).
  // Hide the affordance for users lacking it (hook precedes any early return).
  const canEditConversation = usePermission(Permissions.ConversationsEdit)
  const conversation = Stores.Chat.conversation
  const convId = conversation?.id
  // Reactive read so the pinned state updates when the list refetches.
  const byConv = Stores.Deliverables.byConversation
  if (!convId || !canEditConversation) return null
  const list =
    byConv.get(convId) ?? Stores.Deliverables.getForConversation(convId)
  const isDeliverable = list.some(f => f.id === file.id)

  const toggle = async () => {
    try {
      if (isDeliverable) {
        await Stores.Deliverables.unpin(convId, file.id)
        message.success('Removed from deliverables')
      } else {
        await Stores.Deliverables.pin(convId, file.id, true)
        message.success('Pinned as deliverable')
      }
    } catch (e) {
      console.error('[deliverable-pin] toggle failed', e)
      message.error('Failed to update deliverables')
    }
  }

  return (
    <Button
      variant="ghost"
      size="icon"
      onClick={toggle}
      aria-label={isDeliverable ? 'Remove from deliverables' : 'Pin as deliverable'}
      data-testid="deliverable-pin-toggle"
    >
      {isDeliverable ? <PinOff className="size-3.5" /> : <Pin className="size-3.5" />}
    </Button>
  )
}
