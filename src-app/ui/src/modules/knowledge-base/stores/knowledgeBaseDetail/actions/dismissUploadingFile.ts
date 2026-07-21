import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  (uploadId: string) => {
    set(draft => {
      draft.uploadingFiles.delete(uploadId)
    })
  }
