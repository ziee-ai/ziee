import { createModule } from '@/core'
import { CloudDownloadOutlined } from '@ant-design/icons'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useLlmRepositoryStore } from './store'
import './types' // Import type augmentation
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const LlmRepositorySettings = lazyWithPreload(() => import('./components/LlmRepositorySettings').then(m => ({ default: m.LlmRepositorySettings })))

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
  initialize: () => {
    console.log('LLM Repository module initialized')
  },
  cleanup: () => {
    console.log('LLM Repository module cleanup')
  },
})
