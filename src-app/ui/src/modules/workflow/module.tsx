import { Workflow as WorkflowIcon } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { Stores } from '@ziee/framework/stores'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
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

const GroupSystemWorkflowsWidget = lazyWithPreload(() =>
  import('./widgets/GroupSystemWorkflowsWidget').then(m => ({
    default: m.GroupSystemWorkflowsWidget,
  })),
)

const GroupSystemWorkflowsAssignmentDrawer = lazyWithPreload(() =>
  import('./widgets/GroupSystemWorkflowsAssignmentDrawer').then(m => ({
    default: m.GroupSystemWorkflowsAssignmentDrawer,
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
  ],
  components: [
    {
      id: 'group-system-workflows-assignment-drawer',
      component: GroupSystemWorkflowsAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.GroupSystemWorkflowsAssignment.isOpen),
      order: 100,
    },
  ],
  routes: [
    {
      // A2: workflows live under Settings (mirrors the MCP user page); no
      // top-level sidebar entry anymore.
      path: '/settings/workflows',
      element: WorkflowsList,
      requiresAuth: true,
      permission: Permissions.WorkflowsRead,
      layout: SettingsLayoutDef,
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
    settingsUserPages: [
      {
        id: 'workflows',
        icon: <WorkflowIcon />,
        label: 'Workflows',
        path: 'workflows',
        order: 26,
        permission: Permissions.WorkflowsRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'workflows-admin',
        icon: <WorkflowIcon />,
        label: 'System Workflows',
        path: 'workflows-admin',
        order: 28,
        permission: Permissions.WorkflowsManageSystem,
      },
    ],
    userGroup: [
      {
        order: 40,
        component: GroupSystemWorkflowsWidget,
        // Widget loads group system-workflows (workflows::assign_to_groups).
        permission: Permissions.WorkflowsAssignToGroups,
      },
    ],
  },
})
