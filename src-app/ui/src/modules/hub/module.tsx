import { LayoutGrid } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
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
  // Eligible for ANY hub read perm (mirror HUB_READ_PERM used by the route +
  // sidebar entry) — NOT just HubModelsRead, or an MCP-servers-only user can't
  // load the hub module and loses its sidebar entry / page. `ctx.can` takes a
  // single permission, so OR the three explicitly.
  shouldLoad: (ctx) =>
    ctx.isAuthenticated &&
    (ctx.can(Permissions.HubModelsRead) ||
      ctx.can(Permissions.HubAssistantsRead) ||
      ctx.can(Permissions.HubMCPServersRead)),
  dependencies: ['router'],
  // NOTE: HubCatalog / HubInstalled are page stores (the /hub page). They are
  // registerLazyStore proxies that self-register when the lazy HubPage imports
  // them — so they must NOT be listed here, or registering the hub module (on
  // login, for an eligible user) would pull them onto whatever page you're on.
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
