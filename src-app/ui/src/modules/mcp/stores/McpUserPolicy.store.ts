import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  McpUserPolicy,
  UpdateMcpUserPolicyRequest,
} from '@/api-client/types'
import type { StoreProxy } from '@/core/stores'
import { emitMcpUserPolicyUpdated } from '@/modules/mcp/events/emitters'

/**
 * State shape — REACTIVITY CONTRACT:
 *
 * The `policy` field is a STATE PROPERTY. Reading it via
 * `const { policy } = Stores.McpUserPolicy` from inside a React
 * component installs a `useStore` subscription (see core/stores.ts) —
 * the component re-renders when the policy mutates.
 *
 * There used to be function-typed accessors (`canUserAddMcp`,
 * `allowedTransports`, …) on this store. They were REMOVED because
 * function-typed proxy properties bypass the subscription path and
 * silently produce stale renders. Consumers MUST compute these
 * locally from the `policy` state — that pattern is short and
 * fully reactive:
 *
 *   const { policy } = Stores.McpUserPolicy
 *   const allowedTransports = policy?.allowed_transports ?? []
 *   const canUserAddMcp = allowedTransports.length > 0
 *
 * In non-component code (event handlers, async callbacks) use
 * `Stores.McpUserPolicy.__state.policy` to read the snapshot
 * without installing a subscription.
 */
interface McpUserPolicyState {
  policy: McpUserPolicy | null
  loading: boolean
  error: string | null
  isInitialized: boolean

  // Actions
  load: () => Promise<void>
  update: (req: UpdateMcpUserPolicyRequest) => Promise<void>
  clearError: () => void

  __init__?: { __store__?: () => void }
}

declare module '../../../core/stores' {
  interface RegisteredStores {
    McpUserPolicy: StoreProxy<McpUserPolicyState>
  }
}

export const useMcpUserPolicyStore = create<McpUserPolicyState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      policy: null,
      loading: false,
      error: null,
      isInitialized: false,

      load: async () => {
        if (get().loading) return
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
          // Gracefully degrade on 403 — users without
          // `mcp_servers::read` can't reach this endpoint AND can't
          // do anything with MCP either, so leaving `policy: null`
          // is correct.
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

      __init__: {
        // Loaded on first access — see ../module.tsx for the Stores
        // registration that hooks this in.
        __store__: () => {
          void useMcpUserPolicyStore.getState().load()
        },
      },
    })),
  ),
)
