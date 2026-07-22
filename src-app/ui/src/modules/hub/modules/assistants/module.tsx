import { Bot } from 'lucide-react'
import { createModule } from '@ziee/framework'
import { Permissions } from '@/api-client/permissions'
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
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) =>
    ctx.isAuthenticated &&
    ctx.can(Permissions.HubModelsRead) &&
    (ctx.path === '/hub' || ctx.path.startsWith('/hub/')),
  dependencies: [],
  stores: [],
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
          const { useHubAssistantsStore } = await import('@/modules/hub/modules/assistants/stores/hub-assistants-store')
          await useHubAssistantsStore.getState().refreshFromGitHub()
        },
      },
    ],
  },
})
