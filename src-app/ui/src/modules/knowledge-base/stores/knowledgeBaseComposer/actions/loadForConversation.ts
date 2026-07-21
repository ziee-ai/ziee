import { ApiClient } from '@/api-client'
import type { KnowledgeBaseComposerGet, KnowledgeBaseComposerSet } from '../state'

export default (set: KnowledgeBaseComposerSet, _get: KnowledgeBaseComposerGet) =>
  async (conversationId: string): Promise<void> => {
    try {
      set({ loading: true })
      const kbs = await ApiClient.KnowledgeBase.listConversation({ cid: conversationId })
      set(draft => {
        draft.selectionByConversation.set(
          conversationId,
          new Set((kbs ?? []).map(kb => kb.id)),
        )
        draft.loading = false
      })
    } catch {
      set({ loading: false })
    }
  }
