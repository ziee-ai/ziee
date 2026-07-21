import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  async (id: string, page: number, pageSize: number) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    try {
      set(draft => {
        draft.documentsLoading = true
      })
      const offset = Math.max(0, (page - 1) * pageSize)
      const docs = await ApiClient.KnowledgeBase.listDocuments({
        id,
        limit: pageSize,
        offset,
      })
      set(draft => {
        draft.documents = docs ?? []
        draft.documentsLoading = false
        draft.documentsPage = page
        draft.documentsPageSize = pageSize
      })
    } catch {
      set(draft => {
        draft.documentsLoading = false
      })
    }
  }
