import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  () => {
    set(draft => {
      draft.kb = null
      draft.documents = []
      draft.error = null
      draft.documentsPage = 1
      draft.usage = null
      draft.searchResults = null
      draft.uploadingFiles.clear()
      draft.selectedFileIds.clear()
    })
  }
