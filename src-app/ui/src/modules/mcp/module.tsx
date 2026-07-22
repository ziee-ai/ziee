import { Plug } from 'lucide-react'
import { useOverlayOpen } from '@/core/overlays/overlayVisibility'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
// Deep import (NOT the `@/modules/mcp/stores` barrel): the barrel re-exports
// mcpServer/systemMcpServer/mcpServerDrawer/mcpComposer too, so importing one
// hook from it dragged all of them (incl. the 17 KB mcpComposer) into the
// boot-loaded mcp module chunk.
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
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
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated,
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
  // McpToolCalls (McpServerDrawer "Calls" tab) and McpUserPolicy (read by the
  // MCP settings drawer/card/Add-button + the Hub MCP tab) are both consumed
  // ONLY by page-level surfaces — no always-mounted overlay, sidebar, or chat
  // composer reads them. They're registerLazyStore proxies that self-register
  // (and subscribe to their sync events) when those pages import them, so
  // listing them here — which loaded mcpToolCalls.js + mcpUserPolicy.js on EVERY
  // route at module registration — is intentionally omitted.
  stores: [],
  components: [
    {
      id: 'group-system-mcp-servers-assignment-drawer',
      component: GroupSystemMcpServersAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => useOverlayOpen('group-mcp-assignment')),
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
