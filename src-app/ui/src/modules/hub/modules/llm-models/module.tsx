import { createModule } from '@/core'
import { CloudServerOutlined } from '@ant-design/icons'
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
        icon: <CloudServerOutlined />,
        component: ModelsHubTab,
        order: 10,
        permissions: {
          read: 'hub::models::read',
          refresh: 'hub::models::refresh',
        },
        refresh: async () => {
          await useHubModelsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
