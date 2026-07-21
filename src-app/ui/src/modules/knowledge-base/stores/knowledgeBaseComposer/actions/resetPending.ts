import type { KnowledgeBaseComposerSet } from '../state'
import { pendingKbKey } from '../../kbSelectionKey'

export default (set: KnowledgeBaseComposerSet) =>
  (paneId?: string | null): void => {
    set(draft => {
      const key = pendingKbKey(paneId)
      draft.selectionByConversation.set(key, new Set())
      draft.inheritedByConversation.set(key, new Set())
    })
  }
