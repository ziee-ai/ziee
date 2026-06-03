import type { StoreProxy } from '@/core/stores'
import type {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
  useMcpComposerStore,
} from '@/modules/mcp/stores'
import type { useSystemMcpServerGroupCardStore } from '@/modules/mcp/components/system/McpServerGroupsAssignmentCard.store'
import type { useGroupSystemMcpServersWidgetStore } from '@/modules/mcp/widgets/GroupSystemMcpServersWidget.store'
import type { useGroupSystemMcpServersAssignmentStore } from '@/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.store'
import type { useMcpServerGroupsAssignmentStore } from '@/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.store'
import type { useProjectMcpSettingsStore } from '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'

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
    McpComposer: StoreProxy<
      ReturnType<typeof useMcpComposerStore.getState>
    >
    ProjectMcpSettings: StoreProxy<
      ReturnType<typeof useProjectMcpSettingsStore.getState>
    >
  }
}

export {}
