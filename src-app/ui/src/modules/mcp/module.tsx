import { createModule } from '@/core'
import { Stores } from '@/core/stores'
import { ApiOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
  useMcpComposerStore,
} from '@/modules/mcp/stores'
import { useSystemMcpServerGroupCardStore } from '@/modules/mcp/components/system/McpServerGroupsAssignmentCard.store'
import { useGroupSystemMcpServersWidgetStore } from '@/modules/mcp/widgets/GroupSystemMcpServersWidget.store'
import { useGroupSystemMcpServersAssignmentStore } from '@/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.store'
import { useMcpServerGroupsAssignmentStore } from '@/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.store'
import { useProjectMcpSettingsStore } from '@/modules/mcp/project-extension/stores/ProjectMcpSettings.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import '@/modules/mcp/types' // CRITICAL: Import to enable type declaration merging
import '@/modules/mcp/sync' // registerSync('mcp_server') side-effect
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import '@/modules/mcp/project-extension/events/types' // Project↔MCP event declaration merging
import '@/modules/mcp/project-extension/extension' // Side-effect: register project-extension slot contributions

const McpServersSettings = lazyWithPreload(() =>
  import('./components/user/McpServersSettings').then(m => ({
    default: m.McpServersSettings,
  })),
)

const SystemMcpServersPage = lazyWithPreload(() =>
  import('./components/system/SystemMcpServersPage').then(m => ({
    default: m.SystemMcpServersPage,
  })),
)

const GroupSystemMcpServersWidget = lazyWithPreload(() =>
  import('./widgets/GroupSystemMcpServersWidget').then(m => ({
    default: m.GroupSystemMcpServersWidget,
  })),
)

const GroupSystemMcpServersAssignmentDrawer = lazyWithPreload(() =>
  import('./components/system/GroupSystemMcpServersAssignmentDrawer').then(
    m => ({
      default: m.GroupSystemMcpServersAssignmentDrawer,
    }),
  ),
)

const McpServerGroupsAssignmentDrawer = lazyWithPreload(() =>
  import('./components/system/McpServerGroupsAssignmentDrawer').then(m => ({
    default: m.McpServerGroupsAssignmentDrawer,
  })),
)

export default createModule({
  metadata: {
    name: 'mcp',
    version: '1.0.0',
    description: 'Model Context Protocol (MCP) server management',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/mcp-servers',
      element: McpServersSettings,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/mcp-admin',
      element: SystemMcpServersPage,
      requiresAuth: true,
      permission: Permissions.McpServersAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'McpServer',
      store: useMcpStore,
    },
    {
      name: 'SystemMcpServer',
      store: useSystemMcpServersStore,
    },
    {
      name: 'McpServerDrawer',
      store: useMcpServerDrawerStore,
    },
    {
      name: 'GroupSystemMcpServersWidget',
      store: useGroupSystemMcpServersWidgetStore,
    },
    {
      name: 'GroupSystemMcpServersAssignment',
      store: useGroupSystemMcpServersAssignmentStore,
    },
    {
      name: 'SystemMcpServerGroupCard',
      store: useSystemMcpServerGroupCardStore,
    },
    {
      name: 'McpServerGroupsAssignment',
      store: useMcpServerGroupsAssignmentStore,
    },
    {
      name: 'McpComposer',
      store: useMcpComposerStore,
    },
    {
      name: 'ProjectMcpSettings',
      store: useProjectMcpSettingsStore,
    },
  ],
  components: [
    {
      id: 'group-system-mcp-servers-assignment-drawer',
      component: GroupSystemMcpServersAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.GroupSystemMcpServersAssignment.isOpen),
      order: 100,
    },
    {
      id: 'mcp-server-groups-assignment-drawer',
      component: McpServerGroupsAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.McpServerGroupsAssignment.isOpen),
      order: 101,
    },
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'mcp-servers',
        icon: <ApiOutlined />,
        label: 'MCP Servers',
        path: 'mcp-servers',
        order: 25,
      },
    ],
    settingsAdminPages: [
      {
        id: 'mcp-admin',
        icon: <ApiOutlined />,
        label: 'System MCP Servers',
        path: 'mcp-admin',
        order: 25,
        permission: Permissions.McpServersAdminRead,
      },
    ],
    userGroup: [
      {
        order: 20,
        component: GroupSystemMcpServersWidget,
      },
    ],
  },
  initialize: () => {
    console.log('MCP module initialized')
  },
  cleanup: () => {
    console.log('MCP module cleanup')
  },
})
