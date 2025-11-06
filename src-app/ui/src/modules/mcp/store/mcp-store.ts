import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { enableMapSet } from 'immer'
import { ApiClient } from '@/api-client'
import type {
  McpServer,
  CreateMcpServerRequest,
  UpdateMcpServerRequest,
} from '@/api-client/types'
import { useSystemMcpServersStore } from './system-mcp-servers-store'

// Enable Map and Set support in Immer
enableMapSet()

interface McpState {
  // Server data (accessible servers - personal + system from groups)
  servers: McpServer[]
  isInitialized: boolean

  // Loading states
  loading: boolean
  creating: boolean
  updating: boolean
  deleting: boolean

  // Operation-specific loading states
  operationsLoading: Map<string, boolean>

  // Error states
  error: string | null

  // Initialization methods
  __init__: {
    servers: () => Promise<void>
  }
}

export const useMcpStore = create<McpState>()(
  subscribeWithSelector(
    immer(
      (): McpState => ({
        // Server data
        servers: [],
        isInitialized: false,

        // Loading states
        loading: false,
        creating: false,
        updating: false,
        deleting: false,

        // Operation-specific loading states
        operationsLoading: new Map<string, boolean>(),

        // Error states
        error: null,

        // Initialization methods
        __init__: {
          servers: () => loadMcpServers(),
        },
      }),
    ),
  ),
)

// Helper function to update both stores when a server changes
const updateServerInBothStores = (updatedServer: McpServer) => {
  // Update main MCP store (uses immer)
  useMcpStore.setState(draft => {
    const index = draft.servers.findIndex(
      server => server.id === updatedServer.id,
    )
    if (index >= 0) {
      draft.servers[index] = updatedServer
    }
  })

  // Update system MCP servers store if server exists there (doesn't use immer)
  useSystemMcpServersStore.setState(state => {
    const index = state.systemServers.findIndex(
      server => server.id === updatedServer.id,
    )
    if (index >= 0) {
      return {
        ...state,
        systemServers: state.systemServers.map(server =>
          server.id === updatedServer.id ? updatedServer : server
        ),
      }
    }
    return state
  })
}

// Store methods
export const loadMcpServers = async (): Promise<void> => {
  const state = useMcpStore.getState()

  // Check both isInitialized AND loading to prevent duplicate loads
  if (state.isInitialized || state.loading) {
    return
  }

  try {
    useMcpStore.setState(draft => {
      draft.loading = true
      draft.error = null
    })

    const response = await ApiClient.McpServer.listAccessible({})

    useMcpStore.setState(draft => {
      draft.servers = response.servers
      draft.isInitialized = true
      draft.loading = false
      draft.error = null
    })
  } catch (error) {
    console.error('MCP servers loading failed:', error)
    useMcpStore.setState(draft => {
      draft.loading = false
      draft.error =
        error instanceof Error ? error.message : 'Failed to load MCP servers'
    })
    throw error
  }
}

export const createMcpServer = async (
  data: CreateMcpServerRequest,
): Promise<McpServer> => {
  try {
    useMcpStore.setState(draft => {
      draft.creating = true
      draft.error = null
    })

    const newServer = await ApiClient.McpServer.create(data)

    useMcpStore.setState(draft => {
      draft.servers.push(newServer)
      draft.creating = false
    })

    return newServer
  } catch (error) {
    console.error('MCP server creation failed:', error)
    useMcpStore.setState(draft => {
      draft.creating = false
      draft.error =
        error instanceof Error ? error.message : 'Failed to create MCP server'
    })
    throw error
  }
}

export const updateMcpServer = async (
  serverId: string,
  data: UpdateMcpServerRequest,
): Promise<McpServer> => {
  // Set loading for specific server
  useMcpStore.setState(draft => {
    draft.operationsLoading.set(serverId, true)
    draft.error = null
  })

  try {
    const updatedServer = await ApiClient.McpServer.update({
      id: serverId,
      ...data,
    })

    // Update both stores
    updateServerInBothStores(updatedServer)

    useMcpStore.setState(draft => {
      draft.operationsLoading.delete(serverId)
    })

    return updatedServer
  } catch (error) {
    console.error('MCP server update failed:', error)
    useMcpStore.setState(draft => {
      draft.operationsLoading.delete(serverId)
      draft.error =
        error instanceof Error ? error.message : 'Failed to update MCP server'
    })
    throw error
  }
}

export const deleteMcpServer = async (serverId: string): Promise<void> => {
  useMcpStore.setState(draft => {
    draft.operationsLoading.set(serverId, true)
    draft.error = null
  })

  try {
    await ApiClient.McpServer.delete({ id: serverId })

    // Remove from main MCP store (uses immer)
    useMcpStore.setState(draft => {
      draft.servers = draft.servers.filter(server => server.id !== serverId)
      draft.operationsLoading.delete(serverId)
    })

    // Remove from system MCP servers store if it exists there (doesn't use immer)
    useSystemMcpServersStore.setState(state => ({
      ...state,
      systemServers: state.systemServers.filter(
        server => server.id !== serverId,
      ),
    }))
  } catch (error) {
    console.error('MCP server deletion failed:', error)
    useMcpStore.setState(draft => {
      draft.operationsLoading.delete(serverId)
      draft.error =
        error instanceof Error ? error.message : 'Failed to delete MCP server'
    })
    throw error
  }
}

export const getMcpServer = async (serverId: string): Promise<McpServer> => {
  try {
    const server = await ApiClient.McpServer.get({ id: serverId })

    // Update server in both stores
    updateServerInBothStores(server)

    return server
  } catch (error) {
    console.error('Failed to get MCP server:', error)
    throw error
  }
}

export const clearMcpError = () => {
  useMcpStore.setState(draft => {
    draft.error = null
  })
}

// Helper functions
export const getUserServers = (servers: McpServer[]): McpServer[] => {
  return servers.filter(server => !server.is_system)
}

export const getSystemServers = (servers: McpServer[]): McpServer[] => {
  return servers.filter(server => server.is_system)
}

export const getEnabledServers = (servers: McpServer[]): McpServer[] => {
  return servers.filter(server => server.enabled)
}

export const getServersByType = (
  servers: McpServer[],
  transportType: string,
): McpServer[] => {
  return servers.filter(
    server =>
      server.transport_type.toLowerCase() === transportType.toLowerCase(),
  )
}

export const searchServers = (
  servers: McpServer[],
  query: string,
): McpServer[] => {
  if (!query.trim()) return servers

  const searchTerm = query.toLowerCase()
  return servers.filter(
    server =>
      server.name.toLowerCase().includes(searchTerm) ||
      server.display_name.toLowerCase().includes(searchTerm) ||
      server.description?.toLowerCase().includes(searchTerm) ||
      server.transport_type.toLowerCase().includes(searchTerm),
  )
}
