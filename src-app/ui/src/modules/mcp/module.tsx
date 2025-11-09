import { createModule } from '@/core'
import { ApiOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
} from './stores'
import { useSystemMcpServerGroupCardStore } from './components/McpServerGroupsAssignmentCard.store'
import { useGroupSystemMcpServersWidgetStore } from './widgets/GroupSystemMcpServersWidget.store'
import { useGroupSystemMcpServersAssignmentStore } from './components/GroupSystemMcpServersAssignmentDrawer.store'
import { useMcpServerGroupsAssignmentStore } from './components/McpServerGroupsAssignmentDrawer.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types' // CRITICAL: Import to enable type declaration merging
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const McpServersSettings = lazyWithPreload(() =>
  import('./components/McpServersSettings').then(m => ({
    default: m.McpServersSettings,
  })),
)

const SystemMcpServersPage = lazyWithPreload(() =>
  import('./components/admin/SystemMcpServersPage').then(m => ({
    default: m.SystemMcpServersPage,
  })),
)

const GroupSystemMcpServersWidget = lazyWithPreload(() =>
  import('./widgets/GroupSystemMcpServersWidget').then(m => ({
    default: m.GroupSystemMcpServersWidget,
  })),
)

const GroupSystemMcpServersAssignmentDrawer = lazyWithPreload(() =>
  import('./components/GroupSystemMcpServersAssignmentDrawer').then(m => ({
    default: m.GroupSystemMcpServersAssignmentDrawer,
  })),
)

const McpServerGroupsAssignmentDrawer = lazyWithPreload(() =>
  import('./components/McpServerGroupsAssignmentDrawer').then(m => ({
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
  ],
  components: [
    {
      id: 'group-system-mcp-servers-assignment-drawer',
      component: GroupSystemMcpServersAssignmentDrawer,
      order: 100,
    },
    {
      id: 'mcp-server-groups-assignment-drawer',
      component: McpServerGroupsAssignmentDrawer,
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
