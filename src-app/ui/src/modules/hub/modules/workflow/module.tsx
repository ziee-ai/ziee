import { Workflow } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
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
