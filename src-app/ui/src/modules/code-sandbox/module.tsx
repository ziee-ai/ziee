import { createModule } from '@/core'
import { CloudDownloadOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useSandboxEnvironmentsStore } from './stores/SandboxEnvironments.store'
import { useSandboxResourceLimitsStore } from './stores/SandboxResourceLimits.store'
import './types' // CRITICAL: enable store type declaration merging

const SandboxSettingsPage = lazyWithPreload(() =>
  import('./components/SandboxSettingsPage').then(m => ({
    default: m.SandboxSettingsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'code-sandbox',
    version: '1.0.0',
    description: 'Code sandbox rootfs environment management + resource limits',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/sandbox',
      element: SandboxSettingsPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'SandboxEnvironments',
      store: useSandboxEnvironmentsStore,
    },
    {
      name: 'SandboxResourceLimits',
      store: useSandboxResourceLimitsStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'code-sandbox',
        icon: <CloudDownloadOutlined />,
        label: 'Code Sandbox',
        path: 'sandbox',
        order: 26,
        // Either card on the page is enough access to justify showing
        // the menu entry; per-section gates inside the page still
        // hide each card individually.
        permission: {
          anyOf: [
            'code_sandbox::environments::read',
            'code_sandbox::resource_limits::read',
          ],
        },
      },
    ],
  },
})
