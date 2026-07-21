import type { StoreProxy } from '@ziee/framework/stores'
import type {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
  useMcpComposerStore,
  useMcpToolCallsStore,
} from '@/modules/mcp/stores'
import type { useSystemMcpServerGroupCardStore } from '@/modules/mcp/components/system/mcpServerGroupsAssignmentCard'
import type { useGroupSystemMcpServersWidgetStore } from '@/modules/mcp/widgets/groupSystemMcpServersWidget'
import type { useGroupSystemMcpServersAssignmentStore } from '@/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.store'
import type { useProjectMcpSettingsStore } from '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'

declare module '@ziee/framework/stores' {
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
    McpComposer: StoreProxy<
      ReturnType<typeof useMcpComposerStore.getState>
    >
    McpToolCalls: StoreProxy<
      ReturnType<typeof useMcpToolCallsStore.getState>
    >
    ProjectMcpSettings: StoreProxy<
      ReturnType<typeof useProjectMcpSettingsStore.getState>
    >
  }
}

export {}
