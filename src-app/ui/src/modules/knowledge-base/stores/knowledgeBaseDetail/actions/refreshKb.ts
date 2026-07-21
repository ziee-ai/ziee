import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  async (id: string) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    try {
      const kb = await ApiClient.KnowledgeBase.get({ id })
      set(draft => {
        draft.kb = kb
      })
    } catch {
      /* transient */
    }
  }
