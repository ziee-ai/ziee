import { ApiClient } from '@/api-client'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import type { KnowledgeBaseDocument } from '@/api-client/types'

export default (_set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) =>
  async (id: string) => {
    const failed: KnowledgeBaseDocument[] = get().documents.filter(
      d => d.index_status === 'failed' || d.index_status === 'no_text',
    )
    for (const d of failed) {
      try {
        await ApiClient.KnowledgeBase.reindexDocument({
          id,
          file_id: d.file_id,
        })
      } catch {
        /* per-item; keep going */
      }
    }
  }
