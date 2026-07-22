import { useEffect } from 'react'
import { Pin, PinOff } from 'lucide-react'
import { Button, message } from '@ziee/kit'
import { type File as FileEntity } from '@/api-client/types'
import { Permissions } from '@/api-client/permissions'
import { usePermission } from '@/core/permissions'
import { Deliverables } from '@/modules/file/stores/deliverables'
import { Chat } from '@/modules/chat/core/stores/chatBridge'

/**
 * Pin/unpin a file as a deliverable of the ACTIVE conversation. Renders nothing
 * outside a conversation (e.g. the standalone file-preview drawer). Reads the
 * deliverables map reactively so the icon flips live on `sync:deliverable`.
 */
export function DeliverablePinButton({ file }: { file: FileEntity }) {
  // Pin/unpin mutates the conversation's deliverables (`conversations::edit`).
  // Hide the affordance for users lacking it (hook precedes any early return).
  const canEditConversation = usePermission(Permissions.ConversationsEdit)
  const conversation = Chat.conversation
  const convId = conversation?.id

  // Reactive RENDER-SCOPE read of the deliverables map → the pin icon shows the
  // cached state instantly, updates when the async load lands, and flips live on
  // sync:deliverable (the store refetches into a new Map). Reading this proxy
  // inside the effect / a useState initializer (as before) called the proxy's
  // internal hooks OUTSIDE render → React #321 (invalid hook call).
  const byConversation = Deliverables.byConversation
  const list: FileEntity[] = (convId ? byConversation.get(convId) : undefined) ?? []

  // Kick the async load once when there's no cached entry yet. `.$` is the
  // hook-free snapshot read (safe inside an effect); the reactive read above is
  // what re-renders the button when the load populates the map.
  useEffect(() => {
    if (convId && !Deliverables.$.byConversation.get(convId)) {
      void Deliverables.getForConversation(convId)
    }
  }, [convId])

  if (!convId || !canEditConversation) return null
  const isDeliverable = list.some(f => f.id === file.id)

  const toggle = async () => {
    try {
      if (isDeliverable) {
        await Deliverables.unpin(convId, file.id)
        message.success('Removed from deliverables')
      } else {
        await Deliverables.pin(convId, file.id, true)
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
