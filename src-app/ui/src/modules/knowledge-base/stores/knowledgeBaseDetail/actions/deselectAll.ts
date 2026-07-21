import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  () => {
    set(draft => {
      draft.selectedFileIds.clear()
    })
  }
