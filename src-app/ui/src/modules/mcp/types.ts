import type { StoreProxy } from '@/core/stores'
import type {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
  useGroupSystemMcpServersWidgetStore,
  useGroupSystemMcpServersAssignmentStore,
  useSystemMcpServerGroupCardStore,
  useMcpServerGroupsAssignmentStore,
} from './stores'

declare module '@/core/stores' {
  interface RegisteredStores {
    McpServer: StoreProxy<ReturnType<typeof useMcpStore.getState>>
    SystemMcpServer: StoreProxy<ReturnType<typeof useSystemMcpServersStore.getState>>
    McpServerDrawer: StoreProxy<ReturnType<typeof useMcpServerDrawerStore.getState>>
    GroupSystemMcpServersWidget: StoreProxy<
      ReturnType<typeof useGroupSystemMcpServersWidgetStore.getState>
    >
    GroupSystemMcpServersAssignment: StoreProxy<
      ReturnType<typeof useGroupSystemMcpServersAssignmentStore.getState>
    >
    SystemMcpServerGroupCard: StoreProxy<
      ReturnType<typeof useSystemMcpServerGroupCardStore.getState>
    >
    McpServerGroupsAssignment: StoreProxy<
      ReturnType<typeof useMcpServerGroupsAssignmentStore.getState>
    >
  }
}

export {}
