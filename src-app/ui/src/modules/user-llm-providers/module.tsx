import { createModule } from '@/core'
import { ApiOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useUserLlmProvidersStore } from './UserLlmProviders.store'
import './types'

const UserLlmProvidersPage = lazyWithPreload(
  () => import('./UserLlmProvidersPage'),
)

export default createModule({
  metadata: {
    name: 'user-llm-providers',
    version: '1.0.0',
    description: 'User LLM provider API key management',
  },
  dependencies: ['router'],
  stores: [
    { name: 'UserLlmProviders', store: useUserLlmProvidersStore },
  ],
  routes: [
    {
      path: '/settings/user-llm-providers',
      element: UserLlmProvidersPage,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'user-llm-providers',
        icon: <ApiOutlined />,
        label: 'LLM Providers',
        path: 'user-llm-providers',
        order: 15,
      },
    ],
  },
})
