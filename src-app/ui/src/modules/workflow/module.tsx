import { NodeIndexOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import {
  useSystemWorkflowStore,
  useWorkflowDrawerStore,
  useWorkflowRunStore,
  useWorkflowStore,
} from '@/modules/workflow/stores'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/workflow/types' // CRITICAL: store declaration merging
import '@/modules/settings/types/SettingsSlots' // settings slot types

const WorkflowsList = lazyWithPreload(() =>
  import('./components/WorkflowsList').then(m => ({
    default: m.WorkflowsList,
  })),
)

const AdminWorkflowsPage = lazyWithPreload(() =>
  import('./components/admin/AdminWorkflowsPage').then(m => ({
    default: m.AdminWorkflowsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'workflow',
    version: '1.0.0',
    description: 'Declarative multi-step LLM workflows',
  },
  dependencies: ['router'],
  stores: [
    { name: 'Workflow', store: useWorkflowStore },
    { name: 'SystemWorkflow', store: useSystemWorkflowStore },
    { name: 'WorkflowRun', store: useWorkflowRunStore },
    { name: 'WorkflowDrawer', store: useWorkflowDrawerStore },
  ],
  routes: [
    {
      path: '/workflows',
      element: WorkflowsList,
      requiresAuth: true,
      permission: Permissions.WorkflowsRead,
      layout: AppLayoutDef,
    },
    {
      path: '/settings/workflows-admin',
      element: AdminWorkflowsPage,
      requiresAuth: true,
      permission: Permissions.WorkflowsManageSystem,
      layout: SettingsLayoutDef,
    },
  ],
  slots: {
    sidebarNavigation: [
      {
        id: 'workflows',
        icon: <NodeIndexOutlined />,
        label: 'Workflows',
        path: '/workflows',
        order: 85,
        permission: Permissions.WorkflowsRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'workflows-admin',
        icon: <NodeIndexOutlined />,
        label: 'System Workflows',
        path: 'workflows-admin',
        order: 28,
        permission: Permissions.WorkflowsManageSystem,
      },
    ],
  },
})
