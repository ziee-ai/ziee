import type { StoreSet } from '@ziee/framework/store-kit'

export const knowledgeBaseComposerState = {
  /** conversationId (or pending key) → its directly-attached KB ids. */
  selectionByConversation: new Map<string, Set<string>>(),
  /** conversationId (or pending key) → KB ids inherited (read-only) from its project. */
  inheritedByConversation: new Map<string, Set<string>>(),
  loading: false,
}

export type KnowledgeBaseComposerState = typeof knowledgeBaseComposerState
export type KnowledgeBaseComposerSet = StoreSet<KnowledgeBaseComposerState>
export type KnowledgeBaseComposerGet = () => KnowledgeBaseComposerState
