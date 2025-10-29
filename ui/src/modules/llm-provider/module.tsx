import { createModule } from '@/core'
import { CloudServerOutlined } from '@ant-design/icons'
import { LlmProviderSettings } from './components/LlmProviderSettings'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useLlmProviderStore } from './store'
import './types'

export default createModule({
  metadata: {
    name: 'llm-provider',
    version: '1.0.0',
    description: 'LLM provider management',
  },
  routes: [
    {
      path: '/settings/llm-providers/:providerId?',
      element: <LlmProviderSettings />,
      requiresAuth: true,
      layout: SettingsLayout,
    },
  ],
  stores: [
    {
      name: 'LlmProvider',
      store: useLlmProviderStore,
    },
  ],
  settings: [
    {
      id: 'llm-providers',
      icon: <CloudServerOutlined />,
      label: 'LLM Providers',
      path: 'llm-providers',
      section: 'admin',
      order: 21,
    },
  ],
  initialize: () => {
    console.log('LLM Provider module initialized')
  },
  cleanup: () => {
    console.log('LLM Provider module cleanup')
  },
})
