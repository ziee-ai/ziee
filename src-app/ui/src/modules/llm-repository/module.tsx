import { CloudDownload } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useLlmRepositoryDrawerStore } from '@/modules/llm-repository/components/LlmRepositoryDrawer.store'
import { useLlmRepositoryStore } from '@/modules/llm-repository/stores/llmRepository'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import '@/modules/llm-repository/types' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { Stores } from '@ziee/framework/stores'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const LlmRepositorySettings = lazyWithPreload(() =>
  import('./components/LlmRepositorySettings').then(m => ({
    default: m.LlmRepositorySettings,
  })),
)
const LlmRepositoryDrawer = lazyWithPreload(() =>
  import('./components/LlmRepositoryDrawer').then(m => ({
    default: m.LlmRepositoryDrawer,
  })),
)

export default createModule({
  metadata: {
    name: 'llm-repository',
    version: '1.0.0',
    description: 'LLM model repository management',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/llm-repositories',
      element: LlmRepositorySettings,
      requiresAuth: true,
      permission: Permissions.LlmRepositoriesRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'LlmRepository',
      store: useLlmRepositoryStore,
    },
    {
      name: 'LlmRepositoryDrawer',
      store: useLlmRepositoryDrawerStore,
    },
  ],
  components: [
    {
      id: 'llm-repository-drawer',
      component: LlmRepositoryDrawer,
      // Gate: mount ONLY while the drawer is open (mirrors the sibling
      // GroupLlmProvidersAssignmentDrawer). Without this the drawer's chunk +
      // its `GET /api/llm-repositories` fetch fired on EVERY route (incl. the
      // logged-out login page) for a component that is closed 99% of the time.
      shouldMount: () => useDelayedFalse(() => Stores.LlmRepositoryDrawer.open),
      order: 100,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'llm-repositories',
        icon: <CloudDownload />,
        label: 'LLM Repositories',
        path: 'llm-repositories',
        order: 20,
        permission: Permissions.LlmRepositoriesRead,
      },
    ],
  },
  initialize: () => {
    console.log('LLM Repository module initialized')
  },
  cleanup: () => {
    console.log('LLM Repository module cleanup')
  },
})
