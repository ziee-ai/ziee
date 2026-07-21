import { ApiClient } from '@/api-client'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { KnowledgeBaseDetailGet, KnowledgeBaseDetailSet } from '../state'

export default (set: KnowledgeBaseDetailSet, _get: KnowledgeBaseDetailGet) =>
  async () => {
    if (!hasPermissionNow(Permissions.KnowledgeBaseUse)) return
    try {
      const retrievalInfo = await ApiClient.KnowledgeBase.retrievalInfo()
      set(draft => {
        draft.retrievalInfo = retrievalInfo
      })
    } catch {
      /* transient */
    }
  }
