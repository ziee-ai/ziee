import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  async (id: string, query: string) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    const q = query.trim()
    if (!q) {
      set(draft => {
        draft.searchResults = null
      })
      return
    }
    set(draft => {
      draft.searching = true
    })
    try {
      const res = await ApiClient.KnowledgeBase.search({ id, query: q })
      set(draft => {
        draft.searchResults = res
        draft.searching = false
      })
    } catch (error) {
      set(draft => {
        draft.searching = false
        draft.error =
          error instanceof Error ? error.message : 'Search failed'
      })
    }
  }
