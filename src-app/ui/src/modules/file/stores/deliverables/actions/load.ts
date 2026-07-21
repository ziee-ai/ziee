import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { DeliverablesGet, DeliverablesSet } from '../state'
import type { File as FileEntity } from '@/api-client/types'

export default (set: DeliverablesSet, get: DeliverablesGet) =>
  async (conversationId: string): Promise<void> => {
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
