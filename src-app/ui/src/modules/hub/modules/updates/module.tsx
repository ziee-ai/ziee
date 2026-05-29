import { createModule } from '@/core'
import { SyncOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useHubUpdatesStore } from '@/modules/hub/stores/hub-updates-store'

// Admin-only "Updates" hub tab. Lists installed entities (assistants,
// MCP servers, models that came from the hub catalog) whose recorded
// hub_version is behind the current catalog. Backed by
// GET /api/hub/updates (admin-gated server-side too).

const UpdatesHubTab = lazyWithPreload(() =>
  import('./components/UpdatesHubTab').then(m => ({ default: m.UpdatesHubTab })),
)

export default createModule({
  metadata: {
    name: 'hub-updates',
    version: '1.0.0',
    description: 'Hub tab listing installed entities behind the catalog',
  },
  dependencies: [],
  slots: {
    hubTabs: [
      {
        id: 'updates',
        label: 'Updates',
        icon: <SyncOutlined />,
        component: UpdatesHubTab,
        order: 40, // after Models (10), Assistants (20), MCP Servers (30)
        permissions: {
          read: Permissions.HubAdmin,
        },
        refresh: async () => {
          await useHubUpdatesStore.getState().loadUpdates()
        },
      },
    ],
  },
})
