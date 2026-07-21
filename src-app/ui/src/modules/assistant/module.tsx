import { Bot } from 'lucide-react'
import { createModule } from '@ziee/framework'
import { useAssistantDrawerStore } from '@/modules/assistant/components/assistantDrawer'
import {
  useAssistantPickerStore,
  useTemplateAssistantsStore,
  useUserAssistantsStore,
} from '@/modules/assistant/stores'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import '@/modules/assistant/types'
import { Permissions } from '@/api-client/permissions'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const UserAssistantsSettings = lazyWithPreload(() =>
  import('./pages/UserAssistantsSettings').then(m => ({
    default: m.UserAssistantsSettings,
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
      path: '/settings/assistants',
      element: UserAssistantsSettings,
      requiresAuth: true,
      permission: Permissions.AssistantsRead,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/assistant-templates',
      element: AssistantsSettings,
      requiresAuth: true,
      permission: Permissions.AssistantsTemplateRead,
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
      name: 'AssistantPicker',
      store: useAssistantPickerStore,
    },
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'assistants',
        icon: <Bot />,
        label: 'Assistants',
        path: 'assistants',
        order: 20,
        permission: Permissions.AssistantsRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'assistant-templates',
        icon: <Bot />,
        label: 'Assistant Templates',
        path: 'assistant-templates',
        order: 25,
        permission: Permissions.AssistantsTemplateRead,
      },
    ],
  },
  initialize: () => {
    console.log('Assistants module initialized')
  },
})
