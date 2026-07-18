import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'
import { kbKey, pendingKbKey } from './kbSelectionKey'

enableMapSet()

export { PENDING_KB_KEY, pendingKbKey, kbKey } from './kbSelectionKey'

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

    /** Reset THIS pane's pending (new-chat) buffer so a prior chat's selection never
     *  leaks — and, in split view, so one pane switching to a new chat does not wipe
     *  ANOTHER pane's buffered new-chat selection (ITEM-51: per-pane pending key). */
    resetPending: (paneId?: string | null): void => {
      set(draft => {
        draft.selectionByConversation.set(pendingKbKey(paneId), new Set())
        draft.inheritedByConversation.set(pendingKbKey(paneId), new Set())
      })
    },

    /** Load the read-only KBs inherited from a conversation's project (if any). */
    loadInheritedFor: async (
      conversationId: string | null,
      projectId: string | null,
      paneId?: string | null,
    ): Promise<void> => {
      const key = kbKey(conversationId, paneId)
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

    /** Attach a KB to a SPECIFIC conversation (persist if real; buffer under THIS
     *  pane's pending key if new-chat, so split panes don't share the buffer). */
    attachFor: async (
      conversationId: string | null,
      kbId: string,
      paneId?: string | null,
    ): Promise<void> => {
      if (conversationId) {
        await ApiClient.KnowledgeBase.attachConversation({ cid: conversationId, kb_id: kbId })
      }
      set(draft => {
        const key = kbKey(conversationId, paneId)
        const s = draft.selectionByConversation.get(key) ?? new Set<string>()
        s.add(kbId)
        draft.selectionByConversation.set(key, s)
      })
    },

    /** Detach a KB from a SPECIFIC conversation (or THIS pane's pending buffer). */
    detachFor: async (
      conversationId: string | null,
      kbId: string,
      paneId?: string | null,
    ): Promise<void> => {
      if (conversationId) {
        await ApiClient.KnowledgeBase.detachConversation({ cid: conversationId, kb_id: kbId })
      }
      set(draft => {
        const s = draft.selectionByConversation.get(kbKey(conversationId, paneId))
        if (s) s.delete(kbId)
      })
    },

    /**
     * Persist THIS pane's pending (new-chat) selection to a freshly-created
     * conversation, then move the buffer under the real id. Called from
     * `onMessageSent` with the SENDING pane's id (ITEM-51).
     */
    transferPending: async (
      conversationId: string,
      paneId?: string | null,
    ): Promise<void> => {
      const pendingKey = pendingKbKey(paneId)
      const pending = Array.from(get().selectionByConversation.get(pendingKey) ?? [])
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
        draft.selectionByConversation.delete(pendingKey)
      })
    },
  }),
})

export const useKnowledgeBaseComposerStore = KnowledgeBaseComposer.store
