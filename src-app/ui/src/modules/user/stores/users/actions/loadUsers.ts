import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { UsersSet, UsersGet } from '../state'

export default (set: UsersSet, get: UsersGet) =>
  async (page?: number, pageSize?: number): Promise<void> => {
    if (!hasPermissionNow(Permissions.UsersRead)) return
    try {
      const currentState = get()
      const requestPage = page || currentState.currentPage
      const requestPageSize = pageSize || currentState.pageSize
      // Skip if already initialized and loading first page without explicit page.
      if (currentState.isInitialized && currentState.loading && !page) return
      set({ loading: true, error: null })
      const response = await ApiClient.User.list({
        page: requestPage,
        per_page: requestPageSize,
      })
      set({
        users: response.users,
        total: response.total,
        currentPage: response.page,
        pageSize: response.per_page,
        isInitialized: true,
        loading: false,
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load users',
        loading: false,
      })
      throw error
    }
  }
