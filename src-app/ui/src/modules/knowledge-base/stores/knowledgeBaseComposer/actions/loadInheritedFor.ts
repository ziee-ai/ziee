import { ApiClient } from '@/api-client'
import type { KnowledgeBaseComposerGet, KnowledgeBaseComposerSet } from '../state'
import { kbKey } from '../../kbSelectionKey'

export default (set: KnowledgeBaseComposerSet, _get: KnowledgeBaseComposerGet) =>
  async (
    conversationId: string | null,
    projectId: string | null,
    paneId?: string | null,
  ): Promise<void> => {
    const key = kbKey(conversationId, paneId)
    if (!projectId) {
      set(draft => {
        draft.inheritedByConversation.set(key, new Set())
      })
      return
    }
    try {
      const kbs = await ApiClient.KnowledgeBase.listProject({ pid: projectId })
      set(draft => {
        draft.inheritedByConversation.set(key, new Set((kbs ?? []).map(kb => kb.id)))
      })
    } catch {
      /* transient */
    }
  }
