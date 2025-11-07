import { createModule } from '@/core'
import { ApiOutlined } from '@ant-design/icons'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import {
  useMcpStore,
  useSystemMcpServersStore,
  useMcpServerDrawerStore,
  useGroupSystemMcpServersWidgetStore,
  useGroupSystemMcpServersAssignmentStore,
  useServerGroupCardStore,
  useMcpServerGroupsAssignmentStore,
} from './stores'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types' // CRITICAL: Import to enable type declaration merging

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
  routes: [
    {
      path: '/settings/mcp-servers',
      element: McpServersSettings,
      requiresAuth: true,
      layout: SettingsLayout,
    },
    {
      path: '/settings/mcp-admin',
      element: SystemMcpServersPage,
      requiresAuth: true,
      layout: SettingsLayout,
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
      name: 'ServerGroupCard',
      store: useServerGroupCardStore,
    },
    {
      name: 'McpServerGroupsAssignment',
      store: useMcpServerGroupsAssignmentStore,
    },
  ],
  globalComponents: [
    {
      id: 'group-system-mcp-servers-assignment-drawer',
      component: GroupSystemMcpServersAssignmentDrawer,
    },
    {
      id: 'mcp-server-groups-assignment-drawer',
      component: McpServerGroupsAssignmentDrawer,
    },
  ],
  widgets: {
    userGroup: [
      {
        order: 20,
        component: GroupSystemMcpServersWidget,
      },
    ],
  },
  settings: [
    {
      id: 'mcp-servers',
      icon: <ApiOutlined />,
      label: 'MCP Servers',
      path: 'mcp-servers',
      section: 'user',
      order: 25,
    },
    {
      id: 'mcp-admin',
      icon: <ApiOutlined />,
      label: 'System MCP Servers',
      path: 'mcp-admin',
      section: 'admin',
      order: 25,
    },
  ],
  initialize: () => {
    console.log('MCP module initialized')
  },
  cleanup: () => {
    console.log('MCP module cleanup')
  },
})
