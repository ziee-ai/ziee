import { createModule } from '@/core'
import { RobotOutlined } from '@ant-design/icons'
import AppLayout from '@/components/Layout/AppLayout'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import { useUserAssistantsStore, useTemplateAssistantsStore, useAssistantDrawerStore } from './store'
import './types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const UserAssistantsPage = lazyWithPreload(() => import('./pages/UserAssistantsPage').then(m => ({ default: m.UserAssistantsPage })))
const AssistantsSettings = lazyWithPreload(() => import('./pages/AssistantsSettings').then(m => ({ default: m.AssistantsSettings })))

export default createModule({
  metadata: {
    name: 'assistants',
    version: '1.0.0',
    description: 'AI Assistants module for managing user and template assistants',
  },
  routes: [
    {
      path: '/assistants',
      element: UserAssistantsPage,
      requiresAuth: true,
      layout: AppLayout,
    },
    {
      path: '/settings/assistants',
      element: AssistantsSettings,
      requiresAuth: true,
      layout: SettingsLayout,
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
  ],
  sidebar: {
    tools: [
      {
        id: 'assistants',
        icon: <RobotOutlined />,
        label: 'Assistants',
        path: '/assistants',
        order: 20,
      },
    ],
  },
  settings: [
    {
      id: 'assistants',
      icon: <RobotOutlined />,
      label: 'Assistants',
      path: 'assistants',
      section: 'admin',
      order: 25,
    },
  ],
  initialize: () => {
    console.log('Assistants module initialized')
  },
})
