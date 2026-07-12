import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { defineStore } from '@/core/store-kit'
import { PENDING_KB_KEY, kbKey } from './kbSelectionKey'

enableMapSet()

export { PENDING_KB_KEY, kbKey } from './kbSelectionKey'

/**
 * Conversation-scoped composer selection of knowledge bases to ground on.
 *
 * PER-PANE (ITEM-46): the direct + inherited selections are keyed by conversation
 * id (or the pending key for a not-yet-created new chat), so two split panes on
 * different conversations show/attach their OWN KBs — never the focused/last-loaded
 * pane's. (Previously a single flat `selectedKbIds` tied to one global
 * `currentConversationId` cross-contaminated the panes.)
 *
 * Unlike MCP (which snapshots a per-message config), KB attachment is a plain join
 * row: `search_knowledge` resolves the conversation's attached KBs server-side from
 * the conversation id, so the composer only PERSISTS each toggle (attach/detach) —
 * nothing is injected into the send request. The new-chat flow buffers under the
 * pending key; `onMessageSent` calls `transferPending(newId)` once the id is minted.
 */
export const KnowledgeBaseComposer = defineStore('KnowledgeBaseComposer', {
  immer: true,
  state: {
    /** conversationId (or pending key) → its directly-attached KB ids. */
    selectionByConversation: new Map<string, Set<string>>(),
    /** conversationId (or pending key) → KB ids inherited (read-only) from its project. */
    inheritedByConversation: new Map<string, Set<string>>(),
    loading: false,
  },
  actions: (set, get) => ({
    /** Hydrate a conversation's selection from its server-side attachments. */
    loadForConversation: async (conversationId: string): Promise<void> => {
      try {
        set({ loading: true })
        const kbs = await ApiClient.KnowledgeBase.listConversation({ cid: conversationId })
        set(draft => {
          // Writes to THIS conversation's slot; a late resolve after switching
          // conversations only re-sets its own slot (never clobbers another pane's).
          draft.selectionByConversation.set(
            conversationId,
            new Set((kbs ?? []).map(kb => kb.id)),
          )
          draft.loading = false
        })
      } catch {
        set({ loading: false })
      }
    },

    /** Reset the pending (new-chat) buffer so a prior chat's selection never leaks. */
    resetPending: (): void => {
      set(draft => {
        draft.selectionByConversation.set(PENDING_KB_KEY, new Set())
        draft.inheritedByConversation.set(PENDING_KB_KEY, new Set())
      })
    },

    /** Load the read-only KBs inherited from a conversation's project (if any). */
    loadInheritedFor: async (
      conversationId: string | null,
      projectId: string | null,
    ): Promise<void> => {
      const key = kbKey(conversationId)
      if (!projectId) {
        set(draft => {
          draft.inheritedByConversation.set(key, new Set())
        })
        return
      }
      try {
        const kbs = await ApiClient.KnowledgeBase.listProject({ pid: projectId })
        set(draft => {
          draft.inheritedByConversation.set(key, new Set((kbs ?? []).map(kb => kb.id)))
        })
      } catch {
        /* transient */
      }
    },

    /** Attach a KB to a SPECIFIC conversation (persist if real; buffer if new-chat). */
    attachFor: async (conversationId: string | null, kbId: string): Promise<void> => {
      if (conversationId) {
        await ApiClient.KnowledgeBase.attachConversation({ cid: conversationId, kb_id: kbId })
      }
      set(draft => {
        const key = kbKey(conversationId)
        const s = draft.selectionByConversation.get(key) ?? new Set<string>()
        s.add(kbId)
        draft.selectionByConversation.set(key, s)
      })
    },

    /** Detach a KB from a SPECIFIC conversation. */
    detachFor: async (conversationId: string | null, kbId: string): Promise<void> => {
      if (conversationId) {
        await ApiClient.KnowledgeBase.detachConversation({ cid: conversationId, kb_id: kbId })
      }
      set(draft => {
        const s = draft.selectionByConversation.get(kbKey(conversationId))
        if (s) s.delete(kbId)
      })
    },

    /**
     * Persist the pending (new-chat) selection to a freshly-created conversation,
     * then move the buffer under the real id. Called from `onMessageSent`.
     */
    transferPending: async (conversationId: string): Promise<void> => {
      const pending = Array.from(get().selectionByConversation.get(PENDING_KB_KEY) ?? [])
      for (const kbId of pending) {
        try {
          await ApiClient.KnowledgeBase.attachConversation({
            cid: conversationId,
            kb_id: kbId,
          })
        } catch {
          /* best-effort: a failed attach just drops that KB from grounding */
        }
      }
      set(draft => {
        draft.selectionByConversation.set(conversationId, new Set(pending))
        draft.selectionByConversation.delete(PENDING_KB_KEY)
      })
    },
  }),
})

export const useKnowledgeBaseComposerStore = KnowledgeBaseComposer.store
