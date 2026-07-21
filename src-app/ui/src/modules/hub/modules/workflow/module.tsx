import { Workflow } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useHubWorkflowsStore } from '@/modules/hub/modules/workflow/stores/hub-workflows-store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/hub/modules/workflow/types'

const WorkflowsHubTab = lazyWithPreload(() =>
  import('./components/WorkflowsHubTab').then(m => ({
    default: m.WorkflowsHubTab,
  })),
)

export default createModule({
  metadata: {
    name: 'hub-workflow',
    version: '1.0.0',
    description: 'Hub catalog for workflows',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated,
  dependencies: [],
  stores: [{ name: 'HubWorkflows', store: useHubWorkflowsStore }],
  slots: {
    hubTabs: [
      {
        id: 'workflows',
        label: 'Workflows',
        icon: <Workflow />,
        component: WorkflowsHubTab,
        order: 45,
        permissions: {
          read: Permissions.WorkflowsRead,
          refresh: Permissions.HubCatalogManage,
        },
        refresh: async () => {
          await useHubWorkflowsStore.getState().refresh()
        },
      },
    ],
  },
})
