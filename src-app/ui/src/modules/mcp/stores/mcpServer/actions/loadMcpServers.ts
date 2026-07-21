import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import type { McpServerGet, McpServerSet } from '../state'
import doLoadMcpServersFactory from './_doLoadMcpServers'

export default (set: McpServerSet, get: McpServerGet) => {
  const doLoadMcpServers = doLoadMcpServersFactory(set, get)

  return async (page?: number, pageSize?: number): Promise<void> => {
    // Permission-gate the shell-eager-load fetch: AppLayout triggers this
    // store's init on every render regardless of route; skip for users
    // without mcp_servers::read (the request would 403).
    if (!hasPermissionNow(Permissions.McpServersRead)) return
    const state = get()
    const targetPage = page ?? state.currentPage
    const targetPageSize = pageSize ?? state.pageSize
    const { searchTerm, statusFilter } = state
    void doLoadMcpServers(targetPage, targetPageSize, searchTerm, statusFilter)
  }
}
