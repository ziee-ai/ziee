import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import loadDocumentsFactory from './loadDocuments'
import loadRetrievalInfoFactory from './loadRetrievalInfo'
import loadUsageFactory from './loadUsage'

export default (set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) => {
  const loadDocuments = loadDocumentsFactory(set, get)
  const loadRetrievalInfo = loadRetrievalInfoFactory(set, get)
  const loadUsage = loadUsageFactory(set, get)
  return async (id: string) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    try {
      set(draft => {
        draft.loading = true
        draft.error = null
        // New KB — drop any stale progress/selection from the previous one.
        draft.uploadingFiles.clear()
        draft.selectedFileIds.clear()
      })
      const kb = await ApiClient.KnowledgeBase.get({ id })
      set(draft => {
        draft.kb = kb
        draft.loading = false
      })
      await loadDocuments(id)
      void loadRetrievalInfo()
      void loadUsage(id)
    } catch (error) {
      set(draft => {
        draft.error =
          error instanceof Error ? error.message : 'Failed to load'
        draft.loading = false
      })
    }
  }
}
