import { createModule } from '@/core'
import { CloudDownloadOutlined } from '@ant-design/icons'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useLlmRepositoryStore } from './stores/llm-repository-store'
import { useLlmRepositoryDrawerStore } from './components/LlmRepositoryDrawer.store'
import './types' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const LlmRepositorySettings = lazyWithPreload(() => import('./components/LlmRepositorySettings').then(m => ({ default: m.LlmRepositorySettings })))
const LlmRepositoryDrawer = lazyWithPreload(() => import('./components/LlmRepositoryDrawer').then(m => ({ default: m.LlmRepositoryDrawer })))

export default createModule({
  metadata: {
    name: 'llm-repository',
    version: '1.0.0',
    description: 'LLM model repository management',
  },
  routes: [
    {
      path: '/settings/llm-repositories',
      element: LlmRepositorySettings,
      requiresAuth: true,
      layout: SettingsLayout,
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
  settings: [
    {
      id: 'llm-repositories',
      icon: <CloudDownloadOutlined />,
      label: 'LLM Repositories',
      path: 'llm-repositories',
      section: 'admin',
      order: 20,
    },
  ],
  globalComponents: [
    {
      id: 'llm-repository-drawer',
      component: LlmRepositoryDrawer,
    },
  ],
  initialize: () => {
    console.log('LLM Repository module initialized')
  },
  cleanup: () => {
    console.log('LLM Repository module cleanup')
  },
})
