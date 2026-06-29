import { Plug } from 'lucide-react'
import { createModule } from '@/core'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useModelPickerStore } from './ModelPicker.store'
import { useUserLlmProvidersStore } from './UserLlmProviders.store'
import { useUserProviderKeysStore } from './UserProviderKeys.store'
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
    { name: 'ModelPicker', store: useModelPickerStore },
    { name: 'UserProviderKeys', store: useUserProviderKeysStore },
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
        icon: <Plug />,
        label: 'LLM Providers',
        path: 'user-llm-providers',
        order: 15,
      },
    ],
  },
})
