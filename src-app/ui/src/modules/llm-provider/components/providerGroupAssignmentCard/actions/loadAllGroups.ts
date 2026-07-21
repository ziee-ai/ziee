import { ApiClient } from '@/api-client'
import type { ProviderGroupAssignmentCardGet, ProviderGroupAssignmentCardSet } from '../state'

export default (set: ProviderGroupAssignmentCardSet, get: ProviderGroupAssignmentCardGet) =>
  async (): Promise<void> => {
    const state = get()
    if (state.groupsLoading) return
    if (state.groupsInitialized && !state.groupsError) return
    set(s => {
      s.groupsLoading = true
      s.groupsError = null
    })
    try {
      const response = await ApiClient.UserGroup.list({ page: 1, per_page: 1000 })
      set(s => {
        // Defensive: never assign a non-array (downstream reads `.length`/maps).
        s.allGroups = Array.isArray(response.groups) ? response.groups : []
        s.groupsLoading = false
        s.groupsError = null
        s.groupsInitialized = true
      })
    } catch (error) {
      console.error('Failed to load user groups:', error)
      set(s => {
        s.groupsLoading = false
        s.groupsError = error instanceof Error ? error.message : 'Failed to load groups'
      })
      throw error
    }
  }
