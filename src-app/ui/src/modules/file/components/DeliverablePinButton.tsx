import { useEffect, useState } from 'react'
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

  // Lazy-loaded deliverables are async (store-kit wraps sync returns in Promise).
  // Read cached reactively; fall back to async load on first render.
  const [list, setList] = useState<FileEntity[]>(() => {
    if (!convId) return []
    return Deliverables.byConversation.get(convId) ?? []
  })

  useEffect(() => {
    if (!convId) return
    const cached = Deliverables.byConversation.get(convId)
    if (cached) {
      setList(cached)
      return
    }
    // First render with no cached data — trigger async load.
    let cancelled = false
    void Deliverables.getForConversation(convId).then(result => {
      if (!cancelled) setList(result)
    })
    return () => { cancelled = true }
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
