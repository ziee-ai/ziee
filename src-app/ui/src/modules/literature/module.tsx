import { FileSearchOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useLitSearchAdminStore } from './stores/LitSearchAdmin.store'
import './types' // CRITICAL: enable store + panel-renderer type declaration merging

// The screening right-panel + tool-result card register via the auto-discovered
// chat extension at modules/literature/chat-extension/extension.tsx — no import
// needed here.

const LitSearchSettingsPage = lazyWithPreload(() =>
  import('./components/settings/LitSearchSettingsPage').then(m => ({
    default: m.LitSearchSettingsPage,
  })),
)

export default createModule({
  metadata: {
    // Deliberate name: `literature` is the user-facing FEATURE (search +
    // screening), distinct from the backend's technical `lit_search` MCP server
    // / tool id. Admin store + permissions keep the `LitSearch` prefix because
    // they mirror the generated backend `lit_search::*` permission constants.
    name: 'literature',
    version: '1.0.0',
    description: 'Live literature search & screening (admin settings + screening panel)',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/literature',
      element: LitSearchSettingsPage,
      requiresAuth: true,
      permission: Permissions.LitSearchAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'LitSearchAdmin',
      store: useLitSearchAdminStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'literature',
        icon: <FileSearchOutlined />,
        label: 'Literature Search',
        path: 'literature',
        // 29: unused, adjacent to the connected-tools cluster (code-sandbox 26,
        // web-search 27); avoids the workflow module's 28 (deterministic order).
        order: 29,
        permission: Permissions.LitSearchAdminRead,
      },
    ],
  },
})
