import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { WebSearchUserKeysGet, WebSearchUserKeysSet } from '../state'

export default (set: WebSearchUserKeysSet, _get: WebSearchUserKeysGet) =>
  async (): Promise<void> => {
    if (!hasPermissionNow(Permissions.WebSearchUse)) return
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const res = await ApiClient.WebSearch.listUserKeys()
      set(s => {
        s.providers = res.providers
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error ? error.message : 'Failed to load your web search keys'
        s.loading = false
      })
    }
  }
