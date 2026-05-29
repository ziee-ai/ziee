import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  McpServer,
  CreateMcpServerRequest,
  UpdateMcpServerRequest,
  TestMcpConnectionRequest,
  TestMcpConnectionResponse,
} from '@/api-client/types'
import {
  emitGroupSystemMcpServersChanged,
  emitMcpServerCreated,
  emitMcpServerUpdated,
  emitMcpServerDeleted,
} from '@/modules/mcp/events'
import { Stores } from '@/core/stores'

interface SystemMcpServersState {
  // System servers data
  systemServers: McpServer[]
  systemServersTotal: number
  systemServersPage: number
  systemServersPageSize: number
  systemServersInitialized: boolean

  // Loading states
  systemServersLoading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Operation-specific loading states
  operationsLoading: Map<string, boolean>

  // Error states
  systemServersError: string | null

  // Private: Store event unsubscribers for cleanup
  _eventUnsubscribers?: (() => void)[]

  // Initialization methods
  __init__: {
    __store__?: () => void
    systemServers: () => Promise<void>
  }

  // Cleanup hook
  __destroy__?: () => void

  // Custom delay (10 seconds for system stores)
  __destroyDelay__?: number

  // Actions
  loadSystemServers: (page?: number, pageSize?: number) => Promise<void>
  createSystemServer: (data: CreateMcpServerRequest) => Promise<McpServer>
  updateSystemServer: (
    id: string,
    data: UpdateMcpServerRequest,
  ) => Promise<McpServer>
  deleteSystemServer: (id: string) => Promise<void>
  testSystemServerConnection: (
    data: TestMcpConnectionRequest,
  ) => Promise<TestMcpConnectionResponse>
  getServerGroups: (serverId: string) => Promise<string[]>
  assignServerToGroups: (serverId: string, groupIds: string[]) => Promise<void>
  removeServerFromGroup: (serverId: string, groupId: string) => Promise<void>
  updateGroupServers: (groupId: string, serverIds: string[]) => Promise<void>
  getServersForGroup: (groupId: string) => Promise<McpServer[]>
  clearSystemMcpErrors: () => void
  refreshSystemServers: () => Promise<void>
  isServerOperationLoading: (serverId: string, operation?: string) => boolean
  getSystemServerById: (serverId: string) => McpServer | null
  getEnabledSystemServers: () => McpServer[]
  searchSystemServers: (query: string) => McpServer[]
}

export const useSystemMcpServersStore = create<SystemMcpServersState>()(
  subscribeWithSelector(
    (set, get): SystemMcpServersState => ({
      // System servers data
      systemServers: [],
      systemServersTotal: 0,
      systemServersPage: 1,
      systemServersPageSize: 20,
      systemServersInitialized: false,

      // Loading states
      systemServersLoading: false,
      creating: false,
      updating: false,
      deleting: false,

      // Operation-specific loading states
      operationsLoading: new Map<string, boolean>(),

      // Error states
      systemServersError: null,

      // Initialization methods
      __init__: {
        __store__: () => {
          console.log('🚀 SystemMcpServer store initializing...')

          const eventBus = Stores.EventBus
          const unsubscribers: (() => void)[] = []

          // Subscribe to mcp_server.created and track unsubscriber
          unsubscribers.push(
            eventBus.on('mcp_server.created', async event => {
              const { server } = event.data
              // Only add if it's a system server
              if (server.is_system) {
                set(state => ({
                  systemServers: [...state.systemServers, server],
                  systemServersTotal: state.systemServersTotal + 1,
                }))
              }
            }),
          )

          // Subscribe to mcp_server.updated and track unsubscriber
          unsubscribers.push(
            eventBus.on('mcp_server.updated', async event => {
              const { server } = event.data
              // Only update if it's a system server
              if (server.is_system) {
                set(state => ({
                  systemServers: state.systemServers.map(s =>
                    s.id === server.id ? server : s,
                  ),
                }))
              }
            }),
          )

          // Subscribe to mcp_server.deleted and track unsubscriber
          unsubscribers.push(
            eventBus.on('mcp_server.deleted', async event => {
              const { serverId } = event.data
              set(state => ({
                systemServers: state.systemServers.filter(
                  s => s.id !== serverId,
                ),
                systemServersTotal: state.systemServersTotal - 1,
              }))
            }),
          )

          // Store unsubscribers for cleanup
          set({ _eventUnsubscribers: unsubscribers })

          console.log(`✅ Subscribed to ${unsubscribers.length} events`)
        },
        systemServers: () => get().loadSystemServers(),
      },

      // Actions
      loadSystemServers: async (
        page?: number,
        pageSize?: number,
      ): Promise<void> => {
        const state = get()

        if (
          state.systemServersInitialized &&
          state.systemServersLoading &&
          !page
        ) {
          return
        }

        try {
          const requestPage = page || state.systemServersPage
          const requestPageSize = pageSize || state.systemServersPageSize

          set({
            systemServersLoading: true,
            systemServersError: null,
          })

          const response = await ApiClient.McpServerSystem.list({
            page: requestPage,
            per_page: requestPageSize,
          })

          set({
            systemServers: response.servers,
            systemServersTotal: response.total,
            systemServersPage: response.page,
            systemServersPageSize: response.per_page,
            systemServersInitialized: true,
            systemServersLoading: false,
            systemServersError: null,
          })
        } catch (error) {
          console.error('Failed to load system servers:', error)
          set({
            systemServersLoading: false,
            systemServersError:
              error instanceof Error
                ? error.message
                : 'Failed to load system servers',
          })
          throw error
        }
      },

      createSystemServer: async (
        data: CreateMcpServerRequest,
      ): Promise<McpServer> => {
        try {
          set({
            creating: true,
            systemServersError: null,
          })

          const newServer = await ApiClient.McpServerSystem.create(data)

          // Emit event after successful API call
          try {
            await emitMcpServerCreated(newServer)
          } catch (eventError) {
            console.error(
              'Failed to emit mcp server created event:',
              eventError,
            )
          }

          set(state => ({
            systemServers: [...state.systemServers, newServer],
            systemServersTotal: state.systemServersTotal + 1,
            creating: false,
          }))

          return newServer
        } catch (error) {
          console.error('Failed to create system server:', error)
          set({
            creating: false,
            systemServersError:
              error instanceof Error
                ? error.message
                : 'Failed to create system server',
          })
          throw error
        }
      },

      updateSystemServer: async (
        id: string,
        data: UpdateMcpServerRequest,
      ): Promise<McpServer> => {
        try {
          set({
            updating: true,
            systemServersError: null,
          })

          const updatedServer = await ApiClient.McpServerSystem.update({
            id,
            ...data,
          })

          // Emit event after successful API call
          try {
            await emitMcpServerUpdated(updatedServer)
          } catch (eventError) {
            console.error(
              'Failed to emit mcp server updated event:',
              eventError,
            )
          }

          set(state => ({
            systemServers: state.systemServers.map(server =>
              server.id === id ? updatedServer : server,
            ),
            updating: false,
          }))

          return updatedServer
        } catch (error) {
          console.error('Failed to update system server:', error)
          set({
            updating: false,
            systemServersError:
              error instanceof Error
                ? error.message
                : 'Failed to update system server',
          })
          throw error
        }
      },

      deleteSystemServer: async (id: string): Promise<void> => {
        try {
          set({
            deleting: true,
            systemServersError: null,
          })

          await ApiClient.McpServerSystem.delete({ id })

          // Emit event after successful API call
          try {
            await emitMcpServerDeleted(id)
          } catch (eventError) {
            console.error(
              'Failed to emit mcp server deleted event:',
              eventError,
            )
          }

          set(state => ({
            systemServers: state.systemServers.filter(
              server => server.id !== id,
            ),
            systemServersTotal: state.systemServersTotal - 1,
            deleting: false,
          }))
        } catch (error) {
          console.error('Failed to delete system server:', error)
          set({
            deleting: false,
            systemServersError:
              error instanceof Error
                ? error.message
                : 'Failed to delete system server',
          })
          throw error
        }
      },

      // Probe a candidate system-server config (read-only; nothing persisted).
      // Returns { success, message, tool_count } with HTTP 200 even on failure.
      testSystemServerConnection: async (
        data: TestMcpConnectionRequest,
      ): Promise<TestMcpConnectionResponse> => {
        return await ApiClient.McpServerSystem.testConnection(data)
      },

      getServerGroups: async (serverId: string): Promise<string[]> => {
        try {
          const groupIds = await ApiClient.McpServerSystem.getServerGroups({
            id: serverId,
          })
          return groupIds
        } catch (error) {
          console.error('Failed to get server groups:', error)
          throw error
        }
      },

      assignServerToGroups: async (
        serverId: string,
        groupIds: string[],
      ): Promise<void> => {
        try {
          await ApiClient.McpServerSystem.assignServerToGroups({
            id: serverId,
            group_ids: groupIds,
          })
        } catch (error) {
          console.error('Failed to assign server to groups:', error)
          throw error
        }
      },

      removeServerFromGroup: async (
        serverId: string,
        groupId: string,
      ): Promise<void> => {
        try {
          await ApiClient.McpServerSystem.removeServerFromGroup({
            id: serverId,
            group_id: groupId,
          })
        } catch (error) {
          console.error('Failed to remove server from group:', error)
          throw error
        }
      },

      updateGroupServers: async (
        groupId: string,
        serverIds: string[],
      ): Promise<void> => {
        try {
          // Use the group-centric bulk update API endpoint
          await ApiClient.Group.updateSystemServers({
            group_id: groupId,
            server_ids: serverIds,
          })

          // Emit event to invalidate cache
          await emitGroupSystemMcpServersChanged(groupId, serverIds)
        } catch (error) {
          console.error('Failed to update group servers:', error)
          throw error
        }
      },

      getServersForGroup: async (groupId: string): Promise<McpServer[]> => {
        try {
          const allServers = get().systemServers
          const assignedServers: McpServer[] = []

          for (const server of allServers) {
            const groupIds = await ApiClient.McpServerSystem.getServerGroups({
              id: server.id,
            })
            if (groupIds.includes(groupId)) {
              assignedServers.push(server)
            }
          }

          return assignedServers
        } catch (error) {
          console.error('Failed to get servers for group:', error)
          throw error
        }
      },

      clearSystemMcpErrors: () => {
        set({
          systemServersError: null,
        })
      },

      refreshSystemServers: async (): Promise<void> => {
        const { systemServersPage, systemServersPageSize } = get()
        await get().loadSystemServers(systemServersPage, systemServersPageSize)
      },

      isServerOperationLoading: (
        serverId: string,
        operation?: string,
      ): boolean => {
        const { operationsLoading } = get()
        const operationKey = operation ? `${serverId}-${operation}` : serverId
        return operationsLoading.get(operationKey) || false
      },

      getSystemServerById: (serverId: string): McpServer | null => {
        const { systemServers } = get()
        return systemServers.find(server => server.id === serverId) || null
      },

      getEnabledSystemServers: (): McpServer[] => {
        const { systemServers } = get()
        return systemServers.filter(server => server.enabled)
      },

      searchSystemServers: (query: string): McpServer[] => {
        const { systemServers } = get()

        if (!query.trim()) return systemServers

        const searchTerm = query.toLowerCase()
        return systemServers.filter(
          server =>
            server.name.toLowerCase().includes(searchTerm) ||
            server.display_name.toLowerCase().includes(searchTerm) ||
            server.description?.toLowerCase().includes(searchTerm) ||
            server.transport_type.toLowerCase().includes(searchTerm),
        )
      },

      // Auto-cleanup when no components use this store (after delay)
      __destroy__: () => {
        const { _eventUnsubscribers } = get()

        console.log('🗑️ Destroying SystemMcpServer store')
        console.log(
          `   Unsubscribing from ${_eventUnsubscribers?.length || 0} events`,
        )

        // Unsubscribe from all events
        _eventUnsubscribers?.forEach(unsub => unsub())

        // Reset to initial state
        set({
          systemServers: [],
          systemServersTotal: 0,
          systemServersPage: 1,
          systemServersPageSize: 20,
          systemServersInitialized: false,
          systemServersLoading: false,
          creating: false,
          updating: false,
          deleting: false,
          operationsLoading: new Map(),
          systemServersError: null,
          _eventUnsubscribers: [],
        })

        console.log('✅ SystemMcpServer store cleaned up')
      },

      // Wait 10 seconds before destroying (users might come back)
      __destroyDelay__: 10000,
    }),
  ),
)
