import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  HubMCPServer,
  McpServer,
  CreateMcpServerFromHubRequest,
} from '@/api-client/types'
import {
  emitMcpServerCreated,
  emitMcpServerDeleted,
} from '@/modules/mcp/events'
import { Stores } from '@/core/stores'

interface HubMcpServersState {
  servers: HubMCPServer[]
  version: string | null
  loading: boolean
  creating: boolean
  error: string | null

  // Actions
  loadServers: (force?: boolean) => Promise<void>
  refreshFromGitHub: () => Promise<void>
  createFromHub: (request: CreateMcpServerFromHubRequest) => Promise<McpServer>
  /** Install as a system-wide MCP server (is_system=true, no owner).
   *  Backend requires both `hub::mcp_servers::create` and
   *  `mcp_servers_admin::create` permissions; non-admin callers see
   *  a 403. The frontend gates the button on `McpServersAdminCreate`
   *  so the action is hidden when the user lacks the permission.
   *  `replace_existing: true` is honored on the re-install path
   *  (InstalledHubTab) and overrides the duplicate-prevention 409. */
  createSystemFromHub: (
    request: CreateMcpServerFromHubRequest,
  ) => Promise<McpServer>

  // Lazy initialization
  __init__: {
    servers: () => Promise<void>
    __store__?: () => void
  }
  __destroy__?: () => void
}

export const useHubMcpServersStore = create<HubMcpServersState>()(
  subscribeWithSelector(
    immer(
      (set, get): HubMcpServersState => ({
        servers: [],
        version: null,
        loading: false,
        creating: false,
        error: null,

        loadServers: async (force = false) => {
          const state = get()
          if (state.loading && !force) return

          set({ loading: true, error: null })
          try {
            // Load with user's locale
            const locale = 'en' // TODO: Get from user settings
            const servers = await ApiClient.Hub.getMCPServers({ lang: locale })
            const versionInfo = await ApiClient.Hub.getMCPServersVersion()

            set({
              servers,
              version: versionInfo.version,
              loading: false,
            })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to load hub MCP servers',
              loading: false,
            })
          }
        },

        refreshFromGitHub: async () => {
          set({ loading: true, error: null })
          try {
            // Call category-specific refresh endpoint
            const result = await ApiClient.Hub.refreshMCPServers()

            // Reload if updated
            if (result.updated) {
              await get().loadServers()
            }

            set({ loading: false })
          } catch (error: any) {
            set({
              error: error.message || 'Failed to refresh hub MCP servers',
              loading: false,
            })
            throw error
          }
        },

        createFromHub: async (
          request: CreateMcpServerFromHubRequest,
        ): Promise<McpServer> => {
          set({ creating: true, error: null })
          // Snapshot displaced ids BEFORE the call so the
          // `replace_existing` Re-install path can emit
          // `mcp_server.deleted` for them after the new row exists.
          // Without this, the user's MCP server list keeps showing
          // the OLD (now-deleted) row and clicking it 404s.
          const displacedIds: string[] = request.replace_existing
            ? get()
                .servers.find(s => s.name === request.hub_id)
                ?.created_ids?.slice() ?? []
            : []
          try {
            const response = await ApiClient.Hub.createMcpServerFromHub(request)

            // Update the hub MCP server's created_ids directly from response.
            // On `replace_existing` we REPLACE the array (not append) so the
            // stale uuid drops out — the backend deletes the prior user
            // install + the events emitted below propagate the deletion to
            // other stores.
            set(state => {
              const server = state.servers.find(s => s.name === request.hub_id)
              if (server) {
                if (request.replace_existing) {
                  server.created_ids = [response.hub_tracking.entity_id]
                } else {
                  if (!server.created_ids) {
                    server.created_ids = []
                  }
                  server.created_ids.push(response.hub_tracking.entity_id)
                }
              }
              state.creating = false
            })

            // Emit deletion events for the displaced user installs so
            // the UserMcpServers store + settings pages drop the stale
            // rows. Skip the freshly-installed id (defense-in-depth).
            for (const oldId of displacedIds) {
              if (oldId !== response.hub_tracking.entity_id) {
                try {
                  await emitMcpServerDeleted(oldId)
                } catch (e) {
                  console.warn('Failed to emit mcp_server.deleted:', e)
                }
              }
            }

            // Notify downstream caches (UserMcpServers store, settings
            // pages) that a new user MCP server exists. Without this,
            // navigating to /settings/mcp-servers after install doesn't
            // surface the new row until manual refresh.
            try {
              await emitMcpServerCreated(response.server)
            } catch (e) {
              console.warn('Failed to emit mcp_server.created:', e)
            }

            return response.server
          } catch (error: any) {
            set({
              error: error.message || 'Failed to create MCP server from hub',
              creating: false,
            })
            throw error
          }
        },

        createSystemFromHub: async (
          request: CreateMcpServerFromHubRequest,
        ): Promise<McpServer> => {
          set({ creating: true, error: null })
          // Snapshot the ids being displaced BEFORE the call so the
          // `replace_existing` re-install path can emit
          // `mcp_server.deleted` for them after the new row exists.
          // Without this, the admin MCP servers list keeps the OLD
          // (now-deleted) row and clicking it 404s.
          const displacedIds: string[] = request.replace_existing
            ? get()
                .servers.find(s => s.name === request.hub_id)
                ?.created_system_ids?.slice() ?? []
            : []
          try {
            const response =
              await ApiClient.Hub.createSystemMcpServerFromHub(request)

            // Track the install on the hub server so the card can
            // surface a "System Installed" indicator + disable the
            // re-install button. On `replace_existing` we REPLACE the
            // array (not append) so the stale uuid drops out — the
            // backend deletes the old server + the events emitted
            // below propagate the deletion to other stores.
            set(state => {
              const server = state.servers.find(s => s.name === request.hub_id)
              if (server) {
                if (request.replace_existing) {
                  server.created_system_ids = [
                    response.hub_tracking.entity_id,
                  ]
                } else {
                  if (!server.created_system_ids) {
                    server.created_system_ids = []
                  }
                  server.created_system_ids.push(
                    response.hub_tracking.entity_id,
                  )
                }
              }
              state.creating = false
            })

            // Emit deletion events for the displaced system servers so
            // the SystemMcpServers store + admin pages drop the stale
            // rows. Skip the freshly-installed id (defense-in-depth).
            for (const oldId of displacedIds) {
              if (oldId !== response.hub_tracking.entity_id) {
                try {
                  await emitMcpServerDeleted(oldId)
                } catch (e) {
                  console.warn('Failed to emit mcp_server.deleted:', e)
                }
              }
            }

            // Notify downstream caches (SystemMcpServers store, admin
            // MCP page) that a new system server exists.
            try {
              await emitMcpServerCreated(response.server)
            } catch (e) {
              console.warn('Failed to emit mcp_server.created:', e)
            }

            return response.server
          } catch (error: any) {
            set({
              error:
                error.message || 'Failed to create system MCP server from hub',
              creating: false,
            })
            throw error
          }
        },

        __init__: {
          __store__: () => {
            // Same listener clears both user-install and system-install
            // tracking arrays — backend doesn't discriminate scope on
            // delete (single mcp_server.deleted event covers both).
            // Without the system-install branch, deleting a system
            // server from the admin page would leave the hub card's
            // "System Installed" tag + disabled button stuck until a
            // full reload.
            Stores.EventBus.on(
              'mcp_server.deleted',
              event => {
                const { serverId } = event.data
                set(state => {
                  for (const server of state.servers) {
                    if (server.created_ids) {
                      server.created_ids = server.created_ids.filter(
                        id => id !== serverId,
                      )
                    }
                    if (server.created_system_ids) {
                      server.created_system_ids =
                        server.created_system_ids.filter(
                          id => id !== serverId,
                        )
                    }
                  }
                })
              },
              'HubMcpServersStore',
            )
          },
          servers: () => get().loadServers(),
        },

        // Unsubscribe from EventBus on store destroy so listener slots
        // don't accumulate per destroy/re-init cycle. (audit 09 B-9)
        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('HubMcpServersStore')
        },
      }),
    ),
  ),
)
