import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { UserGroupsGet, UserGroupsSet } from '../state'

export default (set: UserGroupsSet, get: UserGroupsGet) =>
  async (page?: number, pageSize?: number) => {
    if (!hasPermissionNow(Permissions.GroupsRead)) return
    try {
      const currentState = get()
      const requestPage = page || currentState.currentPage
      const requestPageSize = pageSize || currentState.pageSize
      // Skip if already initialized and loading first page without explicit page.
      if (currentState.isInitialized && currentState.loadingGroups && !page) return
      set({ loadingGroups: true, error: null })
      const response = await ApiClient.UserGroup.list({
        page: requestPage,
        per_page: requestPageSize,
      })
      set({
        // Guard: a malformed/edge response must not set `groups` undefined —
        // consumers (UserGroupsDrawer) map over it unconditionally.
        groups: Array.isArray(response.groups) ? response.groups : [],
        total: response.total,
        currentPage: response.page,
        pageSize: response.per_page,
        isInitialized: true,
        loadingGroups: false,
      })
    } catch (error) {
      set({
        error: error instanceof Error ? error.message : 'Failed to load groups',
        loadingGroups: false,
      })
      throw error
    }
  }
