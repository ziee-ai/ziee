import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { type McpToolCall, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { Stores } from '@/core/stores'

interface McpToolCallsState {
  // Current page of the active server's tool-call history.
  calls: McpToolCall[]
  total: number
  currentPage: number
  pageSize: number
  // Which server's history is being shown (drives reload-on-sync).
  serverIdFilter: string | null
  // UI-only toggle: hide built-in/loopback server calls (chat fires many).
  hideBuiltIn: boolean

  loading: boolean
  error: string | null

  __init__: {
    __store__?: () => void
  }
  __destroy__?: () => void

  loadCalls: (
    serverId?: string | null,
    page?: number,
    pageSize?: number,
  ) => Promise<void>
  setPage: (page: number, pageSize?: number) => void
  setHideBuiltIn: (hide: boolean) => void
  clearError: () => void
}

export const useMcpToolCallsStore = create<McpToolCallsState>()(
  subscribeWithSelector(
    immer(
      (set, get): McpToolCallsState => ({
        calls: [],
        total: 0,
        currentPage: 1,
        pageSize: 20,
        serverIdFilter: null,
        hideBuiltIn: false,
        loading: false,
        error: null,

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'McpToolCallsStore'
            // Cross-device delivery (`sync:mcp_tool_call`) + missed-event
            // catch-up on reconnect. `loadCalls` self-gates on the same perm
            // the REST endpoint enforces, so a non-holder's reconnect won't 403.
            const reload = () => {
              const s = get()
              void s.loadCalls(s.serverIdFilter, s.currentPage)
            }
            eventBus.on('sync:mcp_tool_call', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
        },

        loadCalls: async (
          serverId?: string | null,
          page?: number,
          pageSize?: number,
        ): Promise<void> => {
          // no-403 invariant: gate on the SAME permission the endpoint enforces
          // (McpServersRead). AppLayout may eagerly init stores regardless of
          // route; a user without the perm simply shows no history.
          if (!hasPermissionNow(Permissions.McpServersRead)) return

          const state = get()
          const nextServer =
            serverId !== undefined ? serverId : state.serverIdFilter
          const serverChanged = (nextServer ?? null) !== state.serverIdFilter
          const nextPage = page ?? state.currentPage
          const nextPageSize = pageSize ?? state.pageSize

          try {
            set(draft => {
              draft.loading = true
              draft.error = null
              draft.serverIdFilter = nextServer ?? null
              // Avoid showing the previous server's rows while B loads.
              if (serverChanged) draft.calls = []
            })

            const response = await ApiClient.McpToolCall.list({
              page: nextPage,
              per_page: nextPageSize,
              ...(nextServer ? { server_id: nextServer } : {}),
              // Server-side filter so pagination total stays consistent.
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
              draft.error =
                error instanceof Error
                  ? error.message
                  : 'Failed to load tool-call history'
            })
          }
        },

        setPage: (page: number, pageSize?: number) => {
          void get().loadCalls(undefined, page, pageSize)
        },

        setHideBuiltIn: (hide: boolean) => {
          set(draft => {
            draft.hideBuiltIn = hide
            draft.currentPage = 1
          })
          // Re-query with the server-side filter so total/pages stay consistent.
          void get().loadCalls(undefined, 1)
        },

        clearError: () => {
          set(draft => {
            draft.error = null
          })
        },

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('McpToolCallsStore')
        },
      }),
    ),
  ),
)
