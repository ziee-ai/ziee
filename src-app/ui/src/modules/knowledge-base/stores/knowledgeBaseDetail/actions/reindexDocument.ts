import { ApiClient } from '@/api-client'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (_set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  async (id: string, fileId: string) => {
    await ApiClient.KnowledgeBase.reindexDocument({ id, file_id: fileId })
  }
