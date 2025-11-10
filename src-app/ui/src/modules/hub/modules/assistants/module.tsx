import { createModule } from '@/core'
import { RobotOutlined } from '@ant-design/icons'
import { useHubAssistantsStore } from './stores/hub-assistants-store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types'

const AssistantsHubTab = lazyWithPreload(() => import('./components/AssistantsHubTab').then(m => ({ default: m.AssistantsHubTab })))

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
        permission: 'hub::assistants::read',
        refresh: async () => {
          await useHubAssistantsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
