import { createModule } from '@/core'
import { RobotOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { useUserAssistantsStore, useTemplateAssistantsStore } from '@/modules/assistants/stores'
import { useAssistantDrawerStore } from '@/modules/assistants/components/AssistantDrawer.store'
import '@/modules/assistants/types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const UserAssistantsPage = lazyWithPreload(() =>
  import('./pages/UserAssistantsPage').then(m => ({
    default: m.UserAssistantsPage,
  })),
)
const AssistantsSettings = lazyWithPreload(() =>
  import('./pages/AssistantsSettings').then(m => ({
    default: m.AssistantsSettings,
  })),
)

export default createModule({
  metadata: {
    name: 'assistants',
    version: '1.0.0',
    description:
      'AI Assistants module for managing user and template assistants',
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
        permission: 'assistant_templates::read',
      },
    ],
  },
  initialize: () => {
    console.log('Assistants module initialized')
  },
})
