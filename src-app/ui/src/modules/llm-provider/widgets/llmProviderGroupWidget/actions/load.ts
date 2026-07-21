import { ApiClient } from '@/api-client'
import type {
  LlmProviderGroupWidgetGet,
  LlmProviderGroupWidgetSet,
} from '../state'

export default (
    set: LlmProviderGroupWidgetSet,
    get: LlmProviderGroupWidgetGet,
  ) =>
  async (force = false) => {
    const groupId = get().groupId
    if (!groupId) return
    if (get().loading && !force) return
    set(d => {
      d.loading = true
      d.error = null
    })
    try {
      const response = await ApiClient.Group.getProviders({ group_id: groupId })
      set(d => {
        // Defensive: never assign a non-array into `providers` — the widget
        // reads `providers.length` unconditionally, so a malformed/empty
        // response ({} or missing field) would crash the whole group row.
        d.providers = Array.isArray(response.providers) ? response.providers : []
        d.loading = false
      })
    } catch (error) {
      console.error(`Failed to load providers for group ${groupId}:`, error)
      set(d => {
        d.loading = false
        d.error =
          error instanceof Error ? error.message : 'Failed to load providers'
      })
    }
  }
