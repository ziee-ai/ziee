/**
 * Desktop Host-Mount module (feature #3, Part B).
 *
 * Registered ONLY in the desktop bundle: mounting a host folder into the code
 * sandbox is only possible where the sandbox runs on the user's machine.
 * Surfaces:
 *   - project detail "Mounted folders" panel (advanced settings slot),
 *   - per-conversation "Mounted folders" header control,
 *   - admin "Host Mount Policy" settings page.
 * Backed by the desktop-only `/api/host-mounts/*` routes. The native folder
 * picker is `Stores.FileDialog.openFolder`.
 */

import { lazy } from 'react'
import { FolderOpenOutlined } from '@ant-design/icons'
import { createModule, type AppModule } from '@ziee/ui-core'
import { SettingsLayoutDef } from '@ziee/ui-core/modules/settings/SettingsLayout'
import { Permissions } from '@/api-client/types'

import { useProjectHostMountsStore } from './project-extension/stores/ProjectHostMounts.store'
import { useConversationHostMountsStore } from './conversation-extension/stores/ConversationHostMounts.store'
import { useHostMountPolicyStore } from './stores/HostMountPolicy.store'
import { ConversationMountsControl } from './conversation-extension/components/ConversationMountsControl'
// Side-effect import: registers the project advanced-settings panel at boot.
import './project-extension/extension'

const HostMountPolicyPage = lazy(() =>
  import('./pages/HostMountPolicyPage').then((m) => ({
    default: m.HostMountPolicyPage,
  })),
)

const hostMountModule: AppModule = createModule({
  metadata: {
    name: 'host-mount-desktop',
    version: '1.0.0',
    description:
      'Desktop-only: mount host folders into the code sandbox (project / conversation / admin policy).',
  },
  dependencies: ['router'],
  stores: [
    { name: 'ProjectHostMounts', store: useProjectHostMountsStore },
    { name: 'ConversationHostMounts', store: useConversationHostMountsStore },
    { name: 'HostMountPolicy', store: useHostMountPolicyStore },
  ],
  routes: [
    {
      path: '/settings/host-mount',
      element: HostMountPolicyPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
      permission: Permissions.HostMountManage,
    },
  ],
  slots: {
    // Admin settings sidebar entry → /settings/host-mount.
    settingsAdminPages: [
      {
        id: 'host-mount-policy',
        icon: <FolderOpenOutlined />,
        label: 'Host Mount Policy',
        path: 'host-mount',
        order: 95,
        permission: Permissions.HostMountManage,
      },
    ],
    // Per-conversation header decoration.
    chatConversationHeaderTrailing: [
      {
        id: 'host-mount-conversation',
        order: 40,
        component: ConversationMountsControl,
      },
    ],
  },
})

export default hostMountModule
