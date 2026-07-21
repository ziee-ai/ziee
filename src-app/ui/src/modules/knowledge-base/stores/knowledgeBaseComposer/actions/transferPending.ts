import { ApiClient } from '@/api-client'
import type { KnowledgeBaseComposerGet, KnowledgeBaseComposerSet } from '../state'
import { pendingKbKey } from '../../kbSelectionKey'

export default (set: KnowledgeBaseComposerSet, get: KnowledgeBaseComposerGet) =>
  async (
    conversationId: string,
    paneId?: string | null,
  ): Promise<void> => {
    const pendingKey = pendingKbKey(paneId)
    const pending = Array.from(get().selectionByConversation.get(pendingKey) ?? [])
    for (const kbId of pending) {
      try {
        await ApiClient.KnowledgeBase.attachConversation({
          cid: conversationId,
          kb_id: kbId,
        })
      } catch {
        /* best-effort: a failed attach just drops that KB from grounding */
      }
    }
    set(draft => {
      draft.selectionByConversation.set(conversationId, new Set(pending))
      draft.selectionByConversation.delete(pendingKey)
    })
  }
