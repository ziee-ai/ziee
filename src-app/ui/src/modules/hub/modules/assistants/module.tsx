import { Bot } from 'lucide-react'
import { createModule } from '@ziee/framework'
import { Permissions } from '@/api-client/types'
import { useHubAssistantsStore } from '@/modules/hub/modules/assistants/stores/hub-assistants-store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/modules/assistants/types'

const AssistantsHubTab = lazyWithPreload(() =>
  import('./components/AssistantsHubTab').then(m => ({
    default: m.AssistantsHubTab,
  })),
)

export default createModule({
  metadata: {
    name: 'hub-assistants',
    version: '1.0.0',
    description: 'Hub catalog for AI assistants',
  },
  dependencies: [],
  stores: [
    {
      name: 'HubAssistants',
      store: useHubAssistantsStore,
    },
  ],
  slots: {
    hubTabs: [
      {
        id: 'assistants',
        label: 'Assistants',
        icon: <Bot />,
        component: AssistantsHubTab,
        order: 20,
        permissions: {
          read: Permissions.HubAssistantsRead,
          refresh: Permissions.HubAssistantsRefresh,
        },
        refresh: async () => {
          await useHubAssistantsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
