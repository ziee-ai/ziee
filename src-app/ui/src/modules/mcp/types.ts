import type { StoreProxy } from '@/core/stores'
import type {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
} from './stores'
import type { useSystemMcpServerGroupCardStore } from './components/system/McpServerGroupsAssignmentCard.store'
import type { useGroupSystemMcpServersWidgetStore } from './widgets/GroupSystemMcpServersWidget.store'
import type { useGroupSystemMcpServersAssignmentStore } from './components/system/GroupSystemMcpServersAssignmentDrawer.store'
import type { useMcpServerGroupsAssignmentStore } from './components/system/McpServerGroupsAssignmentDrawer.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    McpServer: StoreProxy<ReturnType<typeof useMcpStore.getState>>
    SystemMcpServer: StoreProxy<
      ReturnType<typeof useSystemMcpServersStore.getState>
    >
    McpServerDrawer: StoreProxy<
      ReturnType<typeof useMcpServerDrawerStore.getState>
    >
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
