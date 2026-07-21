import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'
import loadDocumentsPageFactory from './loadDocumentsPage'

export default (set: KnowledgeBaseDetailSet, get: KnowledgeBaseDetailGet) => {
  const loadDocumentsPage = loadDocumentsPageFactory(set, get)
  return async (id: string) => {
    await loadDocumentsPage(id, 1, get().documentsPageSize)
  }
}
