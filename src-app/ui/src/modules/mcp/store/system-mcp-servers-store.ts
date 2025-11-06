import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { ApiClient } from '@/api-client'
import type {
  McpServer,
  CreateMcpServerRequest,
  UpdateMcpServerRequest,
} from '@/api-client/types'

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

  // Initialization methods
  __init__: {
    systemServers: () => Promise<void>
  }
}

export const useSystemMcpServersStore = create<SystemMcpServersState>()(
  subscribeWithSelector(
    (): SystemMcpServersState => ({
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
        systemServers: () => loadSystemServers(),
      },
    }),
  ),
)

// System servers management
export const loadSystemServers = async (
  page?: number,
  pageSize?: number,
): Promise<void> => {
  const state = useSystemMcpServersStore.getState()

  if (state.systemServersInitialized && state.systemServersLoading && !page) {
    return
  }

  try {
    const requestPage = page || state.systemServersPage
    const requestPageSize = pageSize || state.systemServersPageSize

    useSystemMcpServersStore.setState({
      systemServersLoading: true,
      systemServersError: null,
    })

    const response = await ApiClient.McpServerSystem.list({
      page: requestPage,
      per_page: requestPageSize,
    })

    useSystemMcpServersStore.setState({
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
    useSystemMcpServersStore.setState({
      systemServersLoading: false,
      systemServersError:
        error instanceof Error ? error.message : 'Failed to load system servers',
    })
    throw error
  }
}

export const createSystemServer = async (
  data: CreateMcpServerRequest,
): Promise<McpServer> => {
  try {
    useSystemMcpServersStore.setState({
      creating: true,
      systemServersError: null,
    })

    const newServer = await ApiClient.McpServerSystem.create(data)

    useSystemMcpServersStore.setState(state => ({
      systemServers: [...state.systemServers, newServer],
      systemServersTotal: state.systemServersTotal + 1,
      creating: false,
    }))

    return newServer
  } catch (error) {
    console.error('Failed to create system server:', error)
    useSystemMcpServersStore.setState({
      creating: false,
      systemServersError:
        error instanceof Error
          ? error.message
          : 'Failed to create system server',
    })
    throw error
  }
}

export const updateSystemServer = async (
  id: string,
  data: UpdateMcpServerRequest,
): Promise<McpServer> => {
  try {
    useSystemMcpServersStore.setState({
      updating: true,
      systemServersError: null,
    })

    const updatedServer = await ApiClient.McpServerSystem.update({ id, ...data })

    useSystemMcpServersStore.setState(state => ({
      systemServers: state.systemServers.map(server =>
        server.id === id ? updatedServer : server,
      ),
      updating: false,
    }))

    return updatedServer
  } catch (error) {
    console.error('Failed to update system server:', error)
    useSystemMcpServersStore.setState({
      updating: false,
      systemServersError:
        error instanceof Error
          ? error.message
          : 'Failed to update system server',
    })
    throw error
  }
}

export const deleteSystemServer = async (id: string): Promise<void> => {
  try {
    useSystemMcpServersStore.setState({
      deleting: true,
      systemServersError: null,
    })

    await ApiClient.McpServerSystem.delete({ id })

    useSystemMcpServersStore.setState(state => ({
      systemServers: state.systemServers.filter(server => server.id !== id),
      systemServersTotal: state.systemServersTotal - 1,
      deleting: false,
    }))
  } catch (error) {
    console.error('Failed to delete system server:', error)
    useSystemMcpServersStore.setState({
      deleting: false,
      systemServersError:
        error instanceof Error
          ? error.message
          : 'Failed to delete system server',
    })
    throw error
  }
}

// Group assignment management
export const getServerGroups = async (serverId: string): Promise<string[]> => {
  try {
    const groupIds = await ApiClient.McpServerSystem.getServerGroups({
      id: serverId,
    })
    return groupIds
  } catch (error) {
    console.error('Failed to get server groups:', error)
    throw error
  }
}

export const assignServerToGroups = async (
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
}

export const removeServerFromGroup = async (
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
}

// Utility functions
export const clearSystemMcpErrors = () => {
  useSystemMcpServersStore.setState({
    systemServersError: null,
  })
}

export const refreshSystemServers = async (): Promise<void> => {
  const { systemServersPage, systemServersPageSize } =
    useSystemMcpServersStore.getState()
  await loadSystemServers(systemServersPage, systemServersPageSize)
}

export const isServerOperationLoading = (
  serverId: string,
  operation?: string,
): boolean => {
  const { operationsLoading } = useSystemMcpServersStore.getState()
  const operationKey = operation ? `${serverId}-${operation}` : serverId
  return operationsLoading.get(operationKey) || false
}

export const getSystemServerById = (serverId: string): McpServer | null => {
  const { systemServers } = useSystemMcpServersStore.getState()
  return systemServers.find(server => server.id === serverId) || null
}

export const getEnabledSystemServers = (): McpServer[] => {
  const { systemServers } = useSystemMcpServersStore.getState()
  return systemServers.filter(server => server.enabled)
}

export const searchSystemServers = (query: string): McpServer[] => {
  const { systemServers } = useSystemMcpServersStore.getState()

  if (!query.trim()) return systemServers

  const searchTerm = query.toLowerCase()
  return systemServers.filter(
    server =>
      server.name.toLowerCase().includes(searchTerm) ||
      server.display_name.toLowerCase().includes(searchTerm) ||
      server.description?.toLowerCase().includes(searchTerm) ||
      server.transport_type.toLowerCase().includes(searchTerm),
  )
}
