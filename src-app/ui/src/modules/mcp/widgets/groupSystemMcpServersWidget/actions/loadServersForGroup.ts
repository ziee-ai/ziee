import { ApiClient } from '@/api-client'
import type { GroupSystemMcpServersWidgetSet, GroupSystemMcpServersWidgetGet } from '../state'
import loadAllServersFactory from './_loadAllServers'
import type { McpServer } from '@/api-client/types'

export default (set: GroupSystemMcpServersWidgetSet, get: GroupSystemMcpServersWidgetGet) =>
  async (groupId: string, force = false): Promise<void> => {
    const state = get()
    const existing = state.groupServers.get(groupId)
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
      s.groupServers.set(groupId, {
        groupId,
        servers: existing?.servers || [],
        loading: true,
        error: null,
        lastFetched: existing?.lastFetched || null,
      })
    })
    try {
      const loadAllServers = loadAllServersFactory(set, get)
      await loadAllServers()
      const allServers = get().allServers
      // For each server, check if it's assigned to this group.
      const assignedServers: McpServer[] = []
      for (const server of allServers) {
        const groupIds = await ApiClient.McpServerSystem.getServerGroups({ id: server.id })
        if (groupIds.includes(groupId)) assignedServers.push(server)
      }
      set(s => {
        s.groupServers.set(groupId, {
          groupId,
          servers: assignedServers,
          loading: false,
          error: null,
          lastFetched: Date.now(),
        })
      })
    } catch (error) {
      console.error(`Failed to load servers for group ${groupId}:`, error)
      set(s => {
        s.groupServers.set(groupId, {
          groupId,
          servers: existing?.servers || [],
          loading: false,
          error: error instanceof Error ? error.message : 'Failed to load servers',
          lastFetched: existing?.lastFetched || null,
        })
      })
      throw error
    }
  }
