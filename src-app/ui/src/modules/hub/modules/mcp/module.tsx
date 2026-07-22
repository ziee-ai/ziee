import { createModule } from '@ziee/framework'
import { Plug } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/modules/mcp/types'
import { McpUserPolicy } from '@/modules/mcp/stores/mcpUserPolicy'

const McpServersHubTab = lazyWithPreload(() =>
  import('./components/McpServersHubTab').then(m => ({
    default: m.McpServersHubTab,
  })),
)

export default createModule({
  metadata: {
    name: 'hub-mcp',
    version: '1.0.0',
    description: 'Hub catalog for MCP servers',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated && ctx.can(Permissions.HubModelsRead),
  dependencies: [],
  stores: [],
  slots: {
    hubTabs: [
      {
        id: 'mcp-servers',
        label: 'MCP Servers',
        icon: <Plug />,
        component: McpServersHubTab,
        order: 30,
        permissions: {
          read: Permissions.HubMCPServersRead,
          refresh: Permissions.HubMCPServersRefresh,
        },
        // Dynamic gate: hide the MCP tab from non-admins when admin
        // policy says users may not install ANY MCP server. Admins
        // (who can install as system) keep the tab regardless of
        // policy so they can install for the system from the hub.
        //
        // Read `policy` via `$` because shouldRender is invoked
        // from HubPage's useMemo callback (not directly inside the
        // component render body), so the function-typed proxy path
        // wouldn't subscribe anyway. HubPage's useMemo deps include
        // `mcpPolicy` (read at component-render scope via the proxy)
        // — THAT's what makes the memo re-run on policy change.
        shouldRender: () => {
          if (hasPermissionNow(Permissions.McpServersAdminCreate)) {
            return true
          }
          const policy = McpUserPolicy.$.policy
          return !!policy && policy.allowed_transports.length > 0
        },
        refresh: async () => {
          const { useHubMcpServersStore } = await import('@/modules/hub/modules/mcp/stores/hub-mcp-servers-store')
          await useHubMcpServersStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
