import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { defineStore } from '@ziee/framework/store-kit'

enableMapSet()

/**
 * Conversation-scoped composer selection of knowledge bases to ground on.
 *
 * Unlike MCP (which snapshots a per-message config), KB attachment is a plain
 * join row: `search_knowledge` resolves the conversation's attached KBs
 * server-side from the conversation id, so the composer only has to PERSIST each
 * toggle (attach/detach) — nothing is injected into the send request.
 *
 * New-conversation flow mirrors McpComposer's pending buffer: before the
 * conversation exists (`currentConversationId === null`) selections are held
 * locally; the chat extension's `onMessageSent` calls `transferPending(newId)`
 * once the id is minted. Existing conversations hydrate via
 * `KnowledgeBase.listConversation` on `onConversationLoad`.
 */
export const KnowledgeBaseComposer = defineStore('KnowledgeBaseComposer', {
  immer: true,
  state: {
    currentConversationId: null as string | null,
    /** Direct conversation attachments (also the pending buffer when id null). */
    selectedKbIds: new Set<string>(),
    /** KBs inherited (read-only) from the conversation's project — shown as
     *  distinct non-removable chips so the active retrieval scope is legible. */
    inheritedKbIds: new Set<string>(),
    loading: false,
  },
  actions: (set, get) => ({
    /** Hydrate the selection from the conversation's server-side attachments. */
    loadForConversation: async (conversationId: string): Promise<void> => {
      try {
        set({ loading: true })
        const kbs = await ApiClient.KnowledgeBase.listConversation({ cid: conversationId })
        set(draft => {
          // Ignore a stale response if the user navigated away mid-fetch.
          if (draft.currentConversationId !== conversationId) return
          draft.selectedKbIds = new Set((kbs ?? []).map(kb => kb.id))
          draft.loading = false
        })
      } catch {
        set({ loading: false })
      }
    },

    /**
     * Point the composer at a conversation. `null` (a fresh new chat) starts
     * with an empty pending buffer so a prior conversation's attachments never
     * leak into a new one; the extension follows a real id with
     * `loadForConversation` to hydrate from the server.
     */
    setCurrentConversation: (conversationId: string | null): void => {
      set(draft => {
        draft.currentConversationId = conversationId
        if (!conversationId) {
          draft.selectedKbIds = new Set()
          draft.inheritedKbIds = new Set()
        }
      })
    },

    /** Load the read-only KBs inherited from the conversation's project (if any). */
    loadInherited: async (projectId: string | null): Promise<void> => {
      // Capture the conversation this load is for; a late resolve after the user
      // switched conversations must not clobber the new one's inherited set.
      const cid = get().currentConversationId
      if (!projectId) {
        set(draft => {
          draft.inheritedKbIds = new Set()
        })
        return
      }
      try {
        const kbs = await ApiClient.KnowledgeBase.listProject({ pid: projectId })
        set(draft => {
          if (draft.currentConversationId !== cid) return
          draft.inheritedKbIds = new Set((kbs ?? []).map(kb => kb.id))
        })
      } catch {
        /* transient */
      }
    },

    /** Attach a KB. Persists immediately for a real conversation; buffers otherwise. */
    attach: async (kbId: string): Promise<void> => {
      const cid = get().currentConversationId
      if (cid) {
        await ApiClient.KnowledgeBase.attachConversation({ cid, kb_id: kbId })
      }
      set(draft => {
        draft.selectedKbIds.add(kbId)
      })
    },

    /** Detach a KB. Persists immediately for a real conversation; buffers otherwise. */
    detach: async (kbId: string): Promise<void> => {
      const cid = get().currentConversationId
      if (cid) {
        await ApiClient.KnowledgeBase.detachConversation({ cid, kb_id: kbId })
      }
      set(draft => {
        draft.selectedKbIds.delete(kbId)
      })
    },

    /**
     * Persist the pending selection to a freshly-created conversation, then
     * adopt it as current (keeping the selection). Called from `onMessageSent`.
     */
    transferPending: async (conversationId: string): Promise<void> => {
      const ids = Array.from(get().selectedKbIds)
      for (const kbId of ids) {
        try {
          await ApiClient.KnowledgeBase.attachConversation({
            cid: conversationId,
            kb_id: kbId,
          })
        } catch {
          /* best-effort: a failed attach just drops that KB from grounding */
        }
      }
      set({ currentConversationId: conversationId })
    },
  }),
})

export const useKnowledgeBaseComposerStore = KnowledgeBaseComposer.store
