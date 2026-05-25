import { createModule } from '@/core'
import { RobotOutlined } from '@ant-design/icons'
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
        icon: <RobotOutlined />,
        component: AssistantsHubTab,
        order: 20,
        permissions: {
          read: 'hub::assistants::read',
          refresh: 'hub::assistants::refresh',
        },
        refresh: async () => {
          await useHubAssistantsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
