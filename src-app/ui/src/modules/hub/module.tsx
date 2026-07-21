import { LayoutGrid } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useHubCatalogStore } from '@/modules/hub/stores/hub-catalog-store'
import { useHubInstalledStore } from '@/modules/hub/stores/hub-installed-store'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/types'

// Hub coordination module
// Sub-modules are auto-discovered from hub/modules/**/module.tsx

const HubPage = lazyWithPreload(() =>
  import('./HubPage').then(m => ({ default: m.HubPage })),
)

// Single source of truth shared between the route gate and the
// sidebar entry. When adding a new hub submodule, list its ::read
// here so both the route and the sidebar entry stay in sync.
const HUB_READ_PERM = {
  anyOf: [
    Permissions.HubModelsRead,
    Permissions.HubAssistantsRead,
    Permissions.HubMCPServersRead,
  ],
}

export default createModule({
  metadata: {
    name: 'hub',
    version: '1.0.0',
    description: 'Hub catalog coordination module',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated && ctx.can(Permissions.HubModelsRead),
  dependencies: ['router'],
  stores: [
    { name: 'HubCatalog', store: useHubCatalogStore },
    { name: 'HubInstalled', store: useHubInstalledStore },
  ],
  routes: [
    {
      path: '/hub/:activeTab?',
      element: HubPage,
      requiresAuth: true,
      permission: HUB_READ_PERM,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'hub',
        icon: <LayoutGrid />,
        label: 'Hub',
        path: '/hub',
        order: 30,
        permission: HUB_READ_PERM,
      },
    ],
  },
})
