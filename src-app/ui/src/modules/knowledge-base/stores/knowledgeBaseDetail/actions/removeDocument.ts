import { ApiClient } from '@/api-client'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import refreshKbFactory from './refreshKb'
import refreshLoadedDocumentsFactory from './refreshLoadedDocuments'

export default (set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) => {
  const refreshKb = refreshKbFactory(set, get)
  const refreshLoadedDocuments = refreshLoadedDocumentsFactory(set, get)
  return async (id: string, fileId: string) => {
    await ApiClient.KnowledgeBase.removeDocument({ id, file_id: fileId })
    set(draft => {
      draft.selectedFileIds.delete(fileId)
    })
    await refreshKb(id)
    // Reload the current page so pagination + the row set stay correct.
    await refreshLoadedDocuments(id)
  }
}
