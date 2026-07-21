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
      await ApiClient.KnowledgeBase.attachConversation({ cid: conversationId, kb_id: kbId })
    }
    set(draft => {
      const key = kbKey(conversationId, paneId)
      const s = draft.selectionByConversation.get(key) ?? new Set<string>()
      s.add(kbId)
      draft.selectionByConversation.set(key, s)
    })
  }
