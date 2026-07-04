import { ApiClient } from '@/api-client'
import { type EnvironmentInfo, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@/core/store-kit'
import type { StoreProxy } from '@/core/stores'

/**
 * Shared catalog of known sandbox flavors + the host command allowlist — backs
 * the MCP user-policy card's flavor Select and the McpServerDrawer system-stdio
 * flavor Select + host command-tier validator. Lazy-loaded on first access;
 * cached for the session. Falls back to canonical lists if the load fails (the
 * saved value is validated server-side against KNOWN_FLAVORS anyway).
 *
 * `selectOptions` is a pre-shaped derived value (label + value) matching the
 * Select props — saves every consumer the .map(...) boilerplate.
 */
const FALLBACK_OPTIONS = [
  { label: 'full', value: 'full' },
  { label: 'minimal', value: 'minimal' },
]

const FALLBACK_HOST_COMMANDS = ['npx', 'uvx', 'python', 'python3', 'node']

function toOptions(flavors: EnvironmentInfo[]): { label: string; value: string }[] {
  return flavors.map(e => ({ label: `${e.flavor} — ${e.description}`, value: e.flavor }))
}

export const SandboxFlavors = defineStore('SandboxFlavors', {
  immer: true,
  state: {
    flavors: [] as EnvironmentInfo[],
    selectOptions: [] as { label: string; value: string }[],
    hostCommands: [] as string[],
    loading: false,
    error: null as string | null,
    isInitialized: false,
  },
  actions: (set, get) => ({
    load: async () => {
      if (get().loading) return
      // GET /api/code-sandbox/flavors is admin-only (mcp_servers_admin::read).
      // A non-admin mounting a component reading this store would 403 — use the
      // fallback labels instead; the saved value is validated server-side.
      if (!hasPermissionNow(Permissions.McpServersAdminRead)) {
        set(state => {
          state.selectOptions = FALLBACK_OPTIONS
          state.hostCommands = FALLBACK_HOST_COMMANDS
          state.isInitialized = true
        })
        return
      }
      set(state => {
        state.loading = true
        state.error = null
      })
      try {
        const res = await ApiClient.CodeSandbox.listFlavors()
        const flavors = res.available ?? []
        const hostCommands = res.host_allowed_commands ?? []
        set(state => {
          state.flavors = flavors
          state.selectOptions = toOptions(flavors)
          state.hostCommands = hostCommands
          state.loading = false
          state.isInitialized = true
        })
      } catch (err: any) {
        // Fall back to canonical labels so the form is usable.
        const errorMessage = err?.message ?? String(err)
        set(state => {
          state.flavors = []
          state.selectOptions = FALLBACK_OPTIONS
          state.hostCommands = FALLBACK_HOST_COMMANDS
          state.loading = false
          state.error = errorMessage
          state.isInitialized = true
        })
      }
    },
  }),
  init: ({ actions }) => {
    void actions.load()
  },
})

export const useSandboxFlavorsStore = SandboxFlavors.store

declare module '../../../core/stores' {
  interface RegisteredStores {
    SandboxFlavors: StoreProxy<ReturnType<typeof SandboxFlavors.store.getState>>
  }
}
