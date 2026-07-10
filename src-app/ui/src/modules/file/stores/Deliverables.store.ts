import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import { type File as FileEntity, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

enableMapSet()

/**
 * A conversation's deliverables: the files the model authored (derived) plus
 * user-pinned files, minus user-hidden ones. Live-updates via `sync:deliverable`
 * (pin/unpin on this or another device).
 */
export const Deliverables = defineStore('Deliverables', {
  immer: true,
  state: {
    byConversation: new Map<string, FileEntity[]>(),
    loadingSet: new Set<string>(),
  },
  actions: (set, get) => {
    const load = async (conversationId: string): Promise<void> => {
      // `sync:reconnect` fires for every store regardless of audience; skip the
      // refetch for users lacking the read perm (the endpoint would 403). Cache an
      // empty list so a render-triggered getForConversation() doesn't re-schedule
      // a no-op load every frame (mirrors FileVersions.loadVersions).
      if (!hasPermissionNow(Permissions.ConversationsRead)) {
        if (!get().byConversation.has(conversationId)) {
          set(s => {
            const m = new Map(s.byConversation)
            m.set(conversationId, [])
            s.byConversation = m
          })
        }
        return
      }
      set(s => {
        const ls = new Set(s.loadingSet)
        ls.add(conversationId)
        s.loadingSet = ls
      })
      try {
        const files = (await ApiClient.File.listDeliverables({
          id: conversationId,
        })) as FileEntity[]
        set(s => {
          const m = new Map(s.byConversation)
          m.set(conversationId, files ?? [])
          s.byConversation = m
          const ls = new Set(s.loadingSet)
          ls.delete(conversationId)
          s.loadingSet = ls
        })
      } catch (e) {
        set(s => {
          const ls = new Set(s.loadingSet)
          ls.delete(conversationId)
          s.loadingSet = ls
        })
        console.error('[Deliverables] load failed', conversationId, e)
      }
    }
    return {
      load,
      /** Render-safe: cached list, triggering a background load on first call. */
      getForConversation: (conversationId: string): FileEntity[] => {
        const cached = get().byConversation.get(conversationId)
        if (!cached && !get().loadingSet.has(conversationId)) {
          Promise.resolve().then(() => load(conversationId))
        }
        return cached ?? []
      },
      /** Pin a file into (pinned=true) or hide it from (pinned=false) the list. */
      pin: async (
        conversationId: string,
        fileId: string,
        pinned = true,
      ): Promise<void> => {
        await ApiClient.File.pinDeliverable({
          id: conversationId,
          file_id: fileId,
          pinned,
        })
        await load(conversationId)
      },
      /** Remove a file's curation (revert to the derived default). */
      unpin: async (conversationId: string, fileId: string): Promise<void> => {
        await ApiClient.File.unpinDeliverable({
          id: conversationId,
          file_id: fileId,
        })
        await load(conversationId)
      },
    }
  },
  init: ({ on, get, actions }) => {
    on('sync:deliverable', (event: { data?: { id?: string } }) => {
      const id = event?.data?.id
      if (id && get().byConversation.has(id)) void actions.load(id)
    })
    on('sync:reconnect', () => {
      Array.from(get().byConversation.keys()).forEach(
        id => void actions.load(id),
      )
    })
  },
})

export const useDeliverablesStore = Deliverables.store
