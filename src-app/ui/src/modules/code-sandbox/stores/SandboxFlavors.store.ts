import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import { type EnvironmentInfo, Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import type { StoreProxy } from '@/core/stores'

/**
 * Shared catalog of known sandbox flavors + the host command
 * allowlist — backs both:
 *   - MCP user-policy card's "User stdio sandbox flavor" Select
 *   - McpServerDrawer's system-stdio "Sandbox Flavor" Select + the
 *     host command-tier validator
 *
 * Lazy-loaded on first access via `__init__.__store__`. Cached for
 * the session (the catalog is process-static; KNOWN_FLAVORS +
 * HOST_ALLOWED_COMMANDS only change on a server restart). If the
 * load fails, falls back to the canonical lists so the form stays
 * usable while the env endpoint is briefly unavailable.
 *
 * `selectOptions` is a pre-shaped derived value (label + value)
 * matching the antd Select props — saves every consumer the
 * .map(...) boilerplate.
 */
interface SandboxFlavorsState {
  flavors: EnvironmentInfo[]
  selectOptions: { label: string; value: string }[]
  hostCommands: string[]
  loading: boolean
  error: string | null
  isInitialized: boolean

  load: () => Promise<void>

  __init__?: { __store__?: () => void }
}

declare module '../../../core/stores' {
  interface RegisteredStores {
    SandboxFlavors: StoreProxy<SandboxFlavorsState>
  }
}

const FALLBACK_OPTIONS = [
  { label: 'full', value: 'full' },
  { label: 'minimal', value: 'minimal' },
]

const FALLBACK_HOST_COMMANDS = ['npx', 'uvx', 'python', 'python3', 'node']

function toOptions(flavors: EnvironmentInfo[]): {
  label: string
  value: string
}[] {
  return flavors.map(e => ({
    label: `${e.flavor} — ${e.description} (~${e.approximate_size_mb} MB)`,
    value: e.flavor,
  }))
}

export const useSandboxFlavorsStore = create<SandboxFlavorsState>()(
  subscribeWithSelector(
    immer((set, get) => ({
      flavors: [],
      selectOptions: [],
      hostCommands: [],
      loading: false,
      error: null,
      isInitialized: false,

      load: async () => {
        if (get().loading) return
        // GET /api/code-sandbox/flavors is admin-only
        // (mcp_servers_admin::read) — it powers the system-MCP form picker
        // + the MCP user-policy admin card. A non-admin that mounts a
        // component reading this store (e.g. the McpServerDrawer on the hub
        // MCP tab) would otherwise 403. Use the fallback labels instead; the
        // saved value is validated server-side against KNOWN_FLAVORS anyway.
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
          // GET /api/code-sandbox/flavors — returns
          // { available: EnvironmentInfo[], host_allowed_commands: string[] }
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
          // Fall back to the canonical labels so the form is usable
          // — the saved value is validated server-side against
          // KNOWN_FLAVORS regardless.
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

      __init__: {
        // Fires on the FIRST access to ANY property of this store
        // (via the proxy's __store__ initializer hook in
        // core/stores.ts). Without this, consumers reading
        // `selectOptions` (the only field most callers want) would
        // never trigger the load because the proxy's per-prop init
        // only fires for a `__init__[prop]` matching that exact
        // name.
        __store__: () => {
          void useSandboxFlavorsStore.getState().load()
        },
      },
    })),
  ),
)
