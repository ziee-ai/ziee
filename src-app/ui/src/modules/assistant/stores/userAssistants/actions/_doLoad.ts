import { ApiClient } from '@/api-client'
import type { UserAssistantsGet, UserAssistantsSet } from '../state'

export default (set: UserAssistantsSet, _get: UserAssistantsGet) =>
  async (page: number, pageSize: number) => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const response = await ApiClient.Assistant.list({
        page,
        limit: pageSize,
      })
      set(s => {
        s.assistants = response?.assistants ?? []
        s.total = response?.total ?? 0
        s.currentPage = page
        s.pageSize = pageSize
        s.isInitialized = true
        s.loading = false
      })
    } catch (error) {
      set(s => {
        s.error =
          error instanceof Error
            ? error.message
            : 'Failed to load assistants'
        s.loading = false
      })
    }
  }
