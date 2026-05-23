import { createModule } from '@/core'
import { CloudDownloadOutlined, ControlOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useSandboxEnvironmentsStore } from './stores/SandboxEnvironments.store'
import { useSandboxResourceLimitsStore } from './stores/SandboxResourceLimits.store'
import './types' // CRITICAL: enable store type declaration merging

const SandboxEnvironmentsPage = lazyWithPreload(() =>
  import('./components/SandboxEnvironmentsPage').then(m => ({
    default: m.SandboxEnvironmentsPage,
  })),
)

const SandboxResourceLimitsPage = lazyWithPreload(() =>
  import('./components/SandboxResourceLimitsPage').then(m => ({
    default: m.SandboxResourceLimitsPage,
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
    {
      path: '/settings/sandbox-resource-limits',
      element: SandboxResourceLimitsPage,
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
        id: 'sandbox-environments',
        icon: <CloudDownloadOutlined />,
        label: 'Sandbox Environments',
        path: 'sandbox-environments',
        order: 26,
      },
      {
        id: 'sandbox-resource-limits',
        icon: <ControlOutlined />,
        label: 'Sandbox Resource Limits',
        path: 'sandbox-resource-limits',
        order: 27,
      },
    ],
  },
})
