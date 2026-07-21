import { ApiClient } from '@/api-client'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import refreshKbFactory from './refreshKb'
import refreshLoadedDocumentsFactory from './refreshLoadedDocuments'

export default (set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) => {
  const refreshKb = refreshKbFactory(set, get)
  const refreshLoadedDocuments = refreshLoadedDocumentsFactory(set, get)
  return async (id: string) => {
    const ids: string[] = Array.from(get().selectedFileIds)
    if (ids.length === 0) return
    for (const fileId of ids) {
      try {
        await ApiClient.KnowledgeBase.removeDocument({ id, file_id: fileId })
      } catch {
        /* per-item failure surfaced by the caller's toast; keep going */
      }
    }
    set(draft => {
      draft.selectedFileIds.clear()
    })
    await refreshKb(id)
    // Reload the current page so pagination + the row set stay correct.
    await refreshLoadedDocuments(id)
  }
}
