import { createModule } from '@/core'
import { CloudDownloadOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { Permissions } from '@/api-client/types'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useSandboxRootfsVersionsStore } from './stores/SandboxRootfsVersions.store'
import { useSandboxResourceLimitsStore } from './stores/SandboxResourceLimits.store'
import './types' // CRITICAL: enable store type declaration merging
import './sync' // registerSync('code_sandbox_settings') side-effect

const SandboxSettingsPage = lazyWithPreload(() =>
  import('./components/SandboxSettingsPage').then(m => ({
    default: m.SandboxSettingsPage,
  })),
)

// Either card on the page is enough access to justify showing the
// menu entry / letting the page render; per-section gates inside
// the page still hide each card individually.
const SANDBOX_READ_PERM = {
  anyOf: [
    Permissions.CodeSandboxEnvironmentsRead,
    Permissions.CodeSandboxResourceLimitsRead,
  ],
}

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
      permission: SANDBOX_READ_PERM,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'SandboxRootfsVersions',
      store: useSandboxRootfsVersionsStore,
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
        permission: SANDBOX_READ_PERM,
      },
    ],
  },
})
