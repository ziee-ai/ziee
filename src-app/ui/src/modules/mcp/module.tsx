import { Plug } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { Stores } from '@ziee/framework/stores'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { useMcpUserPolicyStore } from './stores/mcpUserPolicy'
import { useSystemMcpServerGroupCardStore } from '@/modules/mcp/components/system/mcpServerGroupsAssignmentCard'
import { useProjectMcpSettingsStore } from '@/modules/mcp/project-extension/stores/projectMcpSettings'
import {
  useMcpToolCallsStore,
  } from '@/modules/mcp/stores'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useGroupSystemMcpServersAssignmentStore } from '@/modules/mcp/components/system/groupSystemMcpServersAssignmentDrawer'
import '@/modules/mcp/types' // CRITICAL: Import to enable type declaration merging
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
      // The page's data is backed by `mcp_servers::read`; gate route + slot
      // to match (every sibling user settings page does). A custom group
      // without the grant otherwise saw the menu item + page shell (backend
      // 403s on load).
      permission: Permissions.McpServersRead,
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
    // BOOT-EAGER (always-mounted overlay) — must stay registered.
    { name: 'GroupSystemMcpServersAssignment', store: useGroupSystemMcpServersAssignmentStore },
    {
      name: 'SystemMcpServerGroupCard',
      store: useSystemMcpServerGroupCardStore,
    },
    {
      // Per-server tool-call history (mcp_tool_calls), shown in the
      // McpServerDrawer "Calls" tab. Refetches live on sync:mcp_tool_call.
      name: 'McpToolCalls',
      store: useMcpToolCallsStore,
    },
    {
      name: 'ProjectMcpSettings',
      store: useProjectMcpSettingsStore,
    },
    {
      // Global MCP user-policy (allowed transports + sandbox flavor
      // for user-installed stdio). Loaded on first access; admin
      // edits emit `mcp_user_policy.updated` so the drawer + Add
      // button + Hub tab re-render without a page reload.
      name: 'McpUserPolicy',
      store: useMcpUserPolicyStore,
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
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'mcp-servers',
        icon: <Plug />,
        label: 'MCP Servers',
        path: 'mcp-servers',
        order: 25,
        permission: Permissions.McpServersRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'mcp-admin',
        icon: <Plug />,
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
        // Widget loads system MCP servers + their groups (mcp_servers_admin::read).
        permission: Permissions.McpServersAdminRead,
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
