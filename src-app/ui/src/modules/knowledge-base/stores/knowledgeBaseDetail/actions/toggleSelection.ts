import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  (fileId: string) => {
    set(draft => {
      if (draft.selectedFileIds.has(fileId)) {
        draft.selectedFileIds.delete(fileId)
      } else {
        draft.selectedFileIds.add(fileId)
      }
    })
  }
