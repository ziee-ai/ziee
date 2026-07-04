import { ApiClient } from '@/api-client'
import { type McpToolCall, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'

export const McpToolCalls = defineStore('McpToolCalls', {
  immer: true,
  state: {
    calls: [] as McpToolCall[],
    total: 0,
    currentPage: 1,
    pageSize: 20,
    serverIdFilter: null as string | null,
    hideBuiltIn: false,
    loading: false,
    error: null as string | null,
  },
  actions: (set, get) => {
    const loadCalls = async (serverId?: string | null, page?: number, pageSize?: number) => {
      // no-403 invariant: gate on the SAME permission the endpoint enforces.
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
    return {
      loadCalls,
      setPage: (page: number, pageSize?: number) => {
        void loadCalls(undefined, page, pageSize)
      },
      setHideBuiltIn: (hide: boolean) => {
        set(draft => {
          draft.hideBuiltIn = hide
          draft.currentPage = 1
        })
        void loadCalls(undefined, 1)
      },
      clearError: () =>
        set(draft => {
          draft.error = null
        }),
    }
  },
  init: ({ on, get, actions }) => {
    const reload = () => {
      const s = get()
      void actions.loadCalls(s.serverIdFilter, s.currentPage)
    }
    on('sync:mcp_tool_call', reload)
    on('sync:reconnect', reload)
  },
})

export const useMcpToolCallsStore = McpToolCalls.store
