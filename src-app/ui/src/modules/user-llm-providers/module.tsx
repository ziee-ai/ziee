import { Plug } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useModelPickerStore } from './modelPicker'
import { useUserLlmProvidersStore } from './UserLlmProviders.store'
import { useUserProviderKeysStore } from './userProviderKeys'
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
      // Backed by `user_llm_providers::read` (server user.rs handler); gate
      // route + slot to match every other user settings page.
      permission: Permissions.UserLlmProvidersRead,
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
        permission: Permissions.UserLlmProvidersRead,
      },
    ],
  },
})
