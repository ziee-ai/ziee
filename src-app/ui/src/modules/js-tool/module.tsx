import { Braces } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useJsToolSettingsStore } from './stores/JsToolSettings.store'
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
    {
      name: 'JsToolSettings',
      store: useJsToolSettingsStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'js-tool',
        icon: <Braces />,
        label: 'Programmatic Tools',
        path: 'js-tool',
        order: 27,
        permission: Permissions.JsToolSettingsRead,
      },
    ],
  },
})
