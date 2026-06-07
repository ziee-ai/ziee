import { createModule } from '@/core'
import { Stores } from '@/core/stores'
import { ApiOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { useHubMcpServersStore } from '@/modules/hub/modules/mcp/stores/hub-mcp-servers-store'
import { useMcpServerDetailsDrawerStore } from '@/modules/hub/modules/mcp/components/McpServerDetailsDrawer.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/modules/mcp/types'

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
  dependencies: [],
  stores: [
    {
      name: 'HubMcpServers',
      store: useHubMcpServersStore,
    },
    {
      name: 'McpServerDetailsDrawer',
      store: useMcpServerDetailsDrawerStore,
    },
  ],
  slots: {
    hubTabs: [
      {
        id: 'mcp-servers',
        label: 'MCP Servers',
        icon: <ApiOutlined />,
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
        // Read `policy` via `__state` because shouldRender is invoked
        // from HubPage's useMemo callback (not directly inside the
        // component render body), so the function-typed proxy path
        // wouldn't subscribe anyway. HubPage's useMemo deps include
        // `mcpPolicy` (read at component-render scope via the proxy)
        // — THAT's what makes the memo re-run on policy change.
        shouldRender: () => {
          if (hasPermissionNow(Permissions.McpServersAdminCreate)) {
            return true
          }
          const policy = Stores.McpUserPolicy.__state.policy
          return !!policy && policy.allowed_transports.length > 0
        },
        refresh: async () => {
          await useHubMcpServersStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
