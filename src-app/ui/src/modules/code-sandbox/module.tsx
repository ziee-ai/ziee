import { CloudDownload } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import './types' // CRITICAL: enable store type declaration merging

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
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated && ctx.can(Permissions.CodeSandboxEnvironmentsRead),
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
  // SandboxFlavors (the shared flavor catalog consumed by the MCP user-policy
  // card + McpServerDrawer's stdio flavor Select) is a registerLazyStore proxy —
  // it self-registers when those MCP settings surfaces import it. Listing it here
  // loaded sandboxFlavors.js on EVERY route at module registration; omitted so it
  // loads only where it's actually read.
  stores: [],
  slots: {
    settingsAdminPages: [
      {
        id: 'code-sandbox',
        icon: <CloudDownload />,
        label: 'Code Sandbox',
        path: 'sandbox',
        order: 26,
        permission: SANDBOX_READ_PERM,
      },
    ],
  },
})
