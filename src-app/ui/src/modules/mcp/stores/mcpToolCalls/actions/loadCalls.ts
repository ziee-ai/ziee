import { ApiClient } from '@/api-client'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { McpToolCallsGet, McpToolCallsSet } from '../state'

export default (set: McpToolCallsSet, get: McpToolCallsGet) =>
  async (serverId?: string | null, page?: number, pageSize?: number) => {
    if (!hasPermissionNow(Permissions.McpServersRead)) return
    const state = get()
    const nextServer = serverId !== undefined ? serverId : state.serverIdFilter
    const serverChanged = (nextServer ?? null) !== state.serverIdFilter
    const nextPage = page ?? state.currentPage
    const nextPageSize = pageSize ?? state.pageSize
    try {
      set(draft => {
        draft.loading = true
        draft.error = null
        draft.serverIdFilter = nextServer ?? null
        if (serverChanged) draft.calls = []
      })
      const response = await ApiClient.McpToolCall.list({
        page: nextPage,
        per_page: nextPageSize,
        ...(nextServer ? { server_id: nextServer } : {}),
        ...(state.hideBuiltIn ? { is_built_in: false } : {}),
      })
      set(draft => {
        draft.calls = response.calls
        draft.total = response.total
        draft.currentPage = response.page
        draft.pageSize = response.per_page
        draft.loading = false
      })
    } catch (error) {
      console.error('MCP tool-call history load failed:', error)
      set(draft => {
        draft.loading = false
        draft.error = error instanceof Error ? error.message : 'Failed to load tool-call history'
      })
    }
  }
