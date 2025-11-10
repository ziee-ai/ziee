import { createModule } from '@/core'
import { RobotOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useUserAssistantsStore, useTemplateAssistantsStore } from './stores'
import { useAssistantDrawerStore } from './components/AssistantDrawer.store'
import { useHubAssistantsStore } from './stores/hub-assistants-store'
import './types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import '@/modules/hub/types/HubTabSlot' // Register hub slot types

const UserAssistantsPage = lazyWithPreload(() => import('./pages/UserAssistantsPage').then(m => ({ default: m.UserAssistantsPage })))
const AssistantsSettings = lazyWithPreload(() => import('./pages/AssistantsSettings').then(m => ({ default: m.AssistantsSettings })))
const AssistantsHubTab = lazyWithPreload(() => import('./components/hub/AssistantsHubTab').then(m => ({ default: m.AssistantsHubTab })))

export default createModule({
  metadata: {
    name: 'assistants',
    version: '1.0.0',
    description: 'AI Assistants module for managing user and template assistants',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/assistants',
      element: UserAssistantsPage,
      requiresAuth: true,
      layout: AppLayoutDef,
    },
    {
      path: '/settings/assistants',
      element: AssistantsSettings,
      requiresAuth: true,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'UserAssistants',
      store: useUserAssistantsStore,
    },
    {
      name: 'TemplateAssistants',
      store: useTemplateAssistantsStore,
    },
    {
      name: 'AssistantDrawer',
      store: useAssistantDrawerStore,
    },
    {
      name: 'HubAssistants',
      store: useHubAssistantsStore,
    },
  ],
  slots: {
    sidebarTools: [
      {
        id: 'assistants',
        icon: <RobotOutlined />,
        label: 'Assistants',
        path: '/assistants',
        order: 20,
      },
    ],
    settingsAdminPages: [
      {
        id: 'assistants',
        icon: <RobotOutlined />,
        label: 'Assistants',
        path: 'assistants',
        order: 25,
      },
    ],
    hubTabs: [
      {
        id: 'assistants',
        label: 'Assistants',
        icon: <RobotOutlined />,
        component: AssistantsHubTab,
        order: 20,
        permission: 'hub::assistants::read',
        refresh: async () => {
          await useHubAssistantsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
  initialize: () => {
    console.log('Assistants module initialized')
  },
})
