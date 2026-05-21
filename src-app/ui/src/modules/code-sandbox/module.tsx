import { createModule } from '@/core'
import { CloudDownloadOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useSandboxEnvironmentsStore } from './stores/SandboxEnvironments.store'
import './types' // CRITICAL: enable store type declaration merging

const SandboxEnvironmentsPage = lazyWithPreload(() =>
  import('./components/SandboxEnvironmentsPage').then(m => ({
    default: m.SandboxEnvironmentsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'code-sandbox',
    version: '1.0.0',
    description: 'Code sandbox rootfs environment management',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/sandbox-environments',
      element: SandboxEnvironmentsPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'SandboxEnvironments',
      store: useSandboxEnvironmentsStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'sandbox-environments',
        icon: <CloudDownloadOutlined />,
        label: 'Sandbox Environments',
        path: 'sandbox-environments',
        order: 26,
      },
    ],
  },
})
