import { createModule } from '@/core'
import { CloudDownloadOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useLlmRepositoryStore } from './stores/LlmRepository.store'
import { useLlmRepositoryDrawerStore } from './components/LlmRepositoryDrawer.store'
import './types' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'
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
      order: 100,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'llm-repositories',
        icon: <CloudDownloadOutlined />,
        label: 'LLM Repositories',
        path: 'llm-repositories',
        order: 20,
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
