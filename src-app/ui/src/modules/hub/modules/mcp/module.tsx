import { createModule } from '@/core'
import { ApiOutlined } from '@ant-design/icons'
import { useHubMcpServersStore } from './stores/hub-mcp-servers-store'
import { useMcpServerDetailsDrawerStore } from './components/McpServerDetailsDrawer.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types'

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
        permission: 'hub::mcp_servers::read',
        refresh: async () => {
          await useHubMcpServersStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
