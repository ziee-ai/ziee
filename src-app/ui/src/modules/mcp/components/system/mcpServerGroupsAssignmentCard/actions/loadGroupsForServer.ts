import { ApiClient } from '@/api-client'
import type { McpServerGroupsAssignmentCardGet, McpServerGroupsAssignmentCardSet } from '../state'
import loadAllGroupsFactory from './loadAllGroups'

export default (set: McpServerGroupsAssignmentCardSet, get: McpServerGroupsAssignmentCardGet) => {
  const loadAllGroups = loadAllGroupsFactory(set, get)
  return async (serverId: string, force = false): Promise<void> => {
    const state = get()
    const existing = state.serverGroups.get(serverId)
    if (existing?.loading && !force) return
    if (
      !force &&
      existing?.lastFetched &&
      Date.now() - existing.lastFetched < 30000 &&
      !existing.error
    ) {
      return
    }
    set(s => {
      s.serverGroups.set(serverId, {
        serverId,
        groups: existing?.groups || [],
        loading: true,
        error: null,
        lastFetched: existing?.lastFetched || null,
      })
    })
    try {
      await loadAllGroups()
      const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: serverId })
      const assignedGroups = get().allGroups.filter((g: { id: string }) => groupIds.includes(g.id))
      set(s => {
        s.serverGroups.set(serverId, {
          serverId,
          groups: assignedGroups,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    } catch (error) {
      console.error(`Failed to load groups for server ${serverId}:`, error)
      set(s => {
        s.serverGroups.set(serverId, {
          serverId,
          groups: existing?.groups || [],
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load groups',
          lastFetched: existing?.lastFetched || null,
        })
      })
      throw error
    }
  }
}
