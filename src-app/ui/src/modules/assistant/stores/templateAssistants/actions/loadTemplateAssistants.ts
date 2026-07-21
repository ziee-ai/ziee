import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { TemplateAssistantsGet, TemplateAssistantsSet } from '../state'

export default (set: TemplateAssistantsSet, get: TemplateAssistantsGet) =>
  async (page?: number, pageSize?: number): Promise<void> => {
    if (!hasPermissionNow(Permissions.AssistantsTemplateRead)) return
    try {
      const currentState = get()
      const requestPage = page ?? currentState.currentPage
      const requestPageSize = pageSize ?? currentState.pageSize
      // Skip if already initialized and loading first page without explicit page.
      if (currentState.isInitialized && currentState.loading && !page) return
      set({ loading: true, error: null })
      const response = await ApiClient.AssistantTemplate.list({
        page: requestPage,
        limit: requestPageSize,
      })
      set({
        assistants: response.assistants,
        total: response.total,
        currentPage: requestPage,
        pageSize: requestPageSize,
        isInitialized: true,
        loading: false,
      })
    } catch (error) {
      set({
        error:
          error instanceof Error ? error.message : 'Failed to load template assistants',
        loading: false,
      })
      throw error
    }
  }
