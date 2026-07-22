import { createModule } from '@ziee/framework'
import { LayoutGrid } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

// "Installed" hub tab. Shows every tracked install (models, assistants,
// MCP servers) the user can see — their own user-scoped installs plus,
// for admins, system-wide installs. Each row carries name, install date,
// installed_version vs current_version, and Re-install / Remove actions.
// No permission gate beyond auth — per-row visibility is computed
// server-side from `hub::catalog::read`.

const InstalledHubTab = lazyWithPreload(() =>
  import('./components/InstalledHubTab').then(m => ({
    default: m.InstalledHubTab,
  })),
)

export default createModule({
  metadata: {
    name: 'hub-installed',
    version: '1.0.0',
    description: 'Hub tab listing every tracked install visible to the caller',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated && ctx.can(Permissions.HubModelsRead),
  dependencies: [],
  slots: {
    hubTabs: [
      {
        id: 'installed',
        label: 'Installed',
        icon: <LayoutGrid />,
        component: InstalledHubTab,
        order: 100, // last — after Models(10)/Assistants(20)/MCP(30)/Skills(40)/Workflows(45)
        // Same loose gate as the parent Hub page — if the user can
        // see the Hub at all (any of the three hub-read perms), they
        // can see the Installed tab. The backend handler additionally
        // filters per-row by user_id + admin status.
        permissions: {
          read: {
            anyOf: [
              Permissions.HubModelsRead,
              Permissions.HubAssistantsRead,
              Permissions.HubMCPServersRead,
            ],
          },
        },
        refresh: async () => {
          const { useHubInstalledStore } = await import('@/modules/hub/stores/hub-installed-store')
          await useHubInstalledStore.getState().loadInstalled()
        },
      },
    ],
  },
})
