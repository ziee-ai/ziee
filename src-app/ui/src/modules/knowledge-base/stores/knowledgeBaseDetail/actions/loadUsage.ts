import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  async (id: string) => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    try {
      const usage = await ApiClient.KnowledgeBase.usage({ id })
      set(draft => {
        draft.usage = usage
      })
    } catch {
      /* transient */
    }
  }
