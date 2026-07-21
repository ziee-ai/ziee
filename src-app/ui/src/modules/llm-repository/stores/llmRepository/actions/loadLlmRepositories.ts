import { ApiClient } from '@/api-client'
import type { LlmRepositoryGet, LlmRepositorySet } from '../state'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'

export default (set: LlmRepositorySet, get: LlmRepositoryGet) =>
  async (page?: number, pageSize?: number) => {
    if (!hasPermissionNow(Permissions.LlmRepositoriesRead)) return
    const state = get()
    if (state.loading) return
    const nextPage = page ?? state.currentPage
    const nextPageSize = pageSize ?? state.pageSize
    try {
      set({ loading: true, error: null })
      const response = await ApiClient.LlmRepository.list({
        page: nextPage,
        per_page: nextPageSize,
      })
      set({
        repositories: response.repositories,
        total: response.total,
        currentPage: response.page,
        pageSize: response.per_page,
        isInitialized: true,
        loading: false,
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load repositories',
        loading: false,
      })
      throw error
    }
  }
