import { ApiClient } from '@/api-client'
import type { KnowledgeBaseComposerGet, KnowledgeBaseComposerSet } from '../state'
import { kbKey } from '../../kbSelectionKey'

export default (set: KnowledgeBaseComposerSet, _get: KnowledgeBaseComposerGet) =>
  async (
    conversationId: string | null,
    kbId: string,
    paneId?: string | null,
  ): Promise<void> => {
    if (conversationId) {
      await ApiClient.KnowledgeBase.detachConversation({ cid: conversationId, kb_id: kbId })
    }
    set(draft => {
      const s = draft.selectionByConversation.get(kbKey(conversationId, paneId))
      if (s) s.delete(kbId)
    })
  }
