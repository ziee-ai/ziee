import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import loadDocumentsPageFactory from './loadDocumentsPage'

export default (set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) => {
  const loadDocumentsPage = loadDocumentsPageFactory(set, get)
  return async (id: string) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    const { documentsPage: page, documentsPageSize: size } = get()
    try {
      const offset = Math.max(0, (page - 1) * size)
      const docs = await ApiClient.KnowledgeBase.listDocuments({
        id,
        limit: size,
        offset,
      })
      if ((docs?.length ?? 0) === 0 && page > 1) {
        await loadDocumentsPage(id, page - 1, size)
        return
      }
      set(draft => {
        draft.documents = docs ?? []
      })
    } catch {
      /* transient */
    }
  }
}
