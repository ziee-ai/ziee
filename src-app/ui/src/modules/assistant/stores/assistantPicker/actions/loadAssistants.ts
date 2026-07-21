import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { AssistantPickerGet, AssistantPickerSet } from '../state'

export default (set: AssistantPickerSet, get: AssistantPickerGet) =>
  async (force = false) => {
    // Permission-gate the shell-eager-load fetch — the chat shell accesses the
    // picker regardless of route; without assistants::read the API 403s.
    if (!hasPermissionNow(Permissions.AssistantsRead)) return
    // Only load once unless a sync event forces a refresh.
    if (!force && get().availableAssistants.length > 0) return
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      // Cap at 100 per server-side `limit`; the picker wants everything visible.
      const response = await ApiClient.Assistant.list({ page: 1, limit: 100 })
      set(s => {
        s.availableAssistants = response.assistants
        s.loading = false
      })
    } catch (error: any) {
      set(s => {
        s.error = error.message || 'Failed to load assistants'
        s.loading = false
      })
    }
  }
