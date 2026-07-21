import { Braces } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import './types' // CRITICAL: enable store type declaration merging

const JsToolSettingsPage = lazyWithPreload(() =>
  import('./components/JsToolSettingsPage').then(m => ({
    default: m.JsToolSettingsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'js-tool',
    version: '1.0.0',
    description: 'Admin-configurable resource limits for the built-in run_js tool',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/js-tool',
      element: JsToolSettingsPage,
      requiresAuth: true,
      permission: Permissions.JsToolSettingsRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'js-tool',
        icon: <Braces />,
        label: 'Programmatic Tools',
        path: 'js-tool',
        // 23: a genuinely-free settingsAdminPages slot (27=Web Search + System
        // Skills, 28=System Workflows, 26=Code Sandbox are all taken); keeps a
        // deterministic sidebar sort with no order collision.
        order: 23,
        permission: Permissions.JsToolSettingsRead,
      },
    ],
  },
})
