import { Server } from 'lucide-react'
import { createModule } from '@ziee/framework'
import { Permissions } from '@/api-client/permissions'
import { useHubModelsStore } from '@/modules/hub/modules/llm-models/stores/hub-models-store'
import { useModelDetailsDrawerStore } from '@/modules/hub/modules/llm-models/components/ModelDetailsDrawer.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/modules/llm-models/types'

const ModelsHubTab = lazyWithPreload(() =>
  import('./components/ModelsHubTab').then(m => ({ default: m.ModelsHubTab })),
)

export default createModule({
  metadata: {
    name: 'hub-llm-models',
    version: '1.0.0',
    description: 'Hub catalog for LLM models',
  },
  dependencies: [],
  stores: [
    {
      name: 'HubModels',
      store: useHubModelsStore,
    },
    {
      name: 'ModelDetailsDrawer',
      store: useModelDetailsDrawerStore,
    },
  ],
  slots: {
    hubTabs: [
      {
        id: 'models',
        label: 'Models',
        icon: <Server />,
        component: ModelsHubTab,
        order: 10,
        permissions: {
          read: Permissions.HubModelsRead,
          refresh: Permissions.HubModelsRefresh,
        },
        refresh: async () => {
          await useHubModelsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
