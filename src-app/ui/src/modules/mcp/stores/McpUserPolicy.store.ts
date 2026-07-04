import { ApiClient } from '@/api-client'
import type {
  McpUserPolicy as McpUserPolicyRow,
  UpdateMcpUserPolicyRequest,
} from '@/api-client/types'
import { defineStore } from '@/core/store-kit'
import type { StoreProxy } from '@/core/stores'
import { emitMcpUserPolicyUpdated } from '@/modules/mcp/events/emitters'
import { useAuthStore } from '@/modules/auth/Auth.store'

/**
 * REACTIVITY CONTRACT: `policy` is a STATE PROPERTY. Reading it via
 * `const { policy } = Stores.McpUserPolicy` from a component installs a
 * subscription (re-renders on mutation). Function-typed accessors were REMOVED
 * because function-typed proxy props bypass the subscription path and go stale;
 * compute derived values locally from `policy`:
 *
 *   const { policy } = Stores.McpUserPolicy
 *   const allowedTransports = policy?.allowed_transports ?? []
 *   const canUserAddMcp = allowedTransports.length > 0
 *
 * In non-component code use `Stores.McpUserPolicy.$.policy` for a snapshot.
 */
export const McpUserPolicy = defineStore('McpUserPolicy', {
  immer: true,
  state: {
    policy: null as McpUserPolicyRow | null,
    loading: false,
    error: null as string | null,
    isInitialized: false,
  },
  actions: (set, get) => ({
    load: async () => {
      if (get().loading) return
      // Endpoint is gated on `mcp_servers::read`; short-circuit when the user
      // lacks it (leave policy: null — consumers treat that as "no restrictions
      // surfaced") so the no-403 fixture doesn't flag a missing UI gate.
      const auth = useAuthStore.getState()
      const hasMcpRead =
        auth.user?.is_admin ||
        (auth.permissions ?? []).some(
          p => p === 'mcp_servers::read' || p === '*' || p === 'mcp_servers::*',
        )
      if (!hasMcpRead) {
        set(state => {
          state.isInitialized = true
        })
        return
      }
      set(state => {
        state.loading = true
        state.error = null
      })
      try {
        const policy = await ApiClient.McpUserPolicy.get()
        set(state => {
          state.policy = policy
          state.loading = false
          state.isInitialized = true
        })
      } catch (err: any) {
        // Defensive — gracefully degrade on any unexpected error.
        const errorMessage = err?.message ?? String(err)
        set(state => {
          state.loading = false
          state.error = errorMessage
          state.isInitialized = true
        })
      }
    },
    update: async (req: UpdateMcpUserPolicyRequest) => {
      const policy = await ApiClient.McpUserPolicy.update(req)
      set(state => {
        state.policy = policy
        state.error = null
      })
      await emitMcpUserPolicyUpdated(
        policy.allowed_transports,
        policy.user_stdio_sandbox_flavor ?? null,
      )
    },
    clearError: () => {
      set(state => {
        state.error = null
      })
    },
  }),
  init: ({ actions }) => {
    void actions.load()
  },
})

export const useMcpUserPolicyStore = McpUserPolicy.store

declare module '../../../core/stores' {
  interface RegisteredStores {
    McpUserPolicy: StoreProxy<ReturnType<typeof McpUserPolicy.store.getState>>
  }
}
