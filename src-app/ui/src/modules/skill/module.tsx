import { BookOpen } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useGroupSystemSkillsAssignmentStore } from '@/modules/skill/widgets/groupSystemSkillsAssignmentDrawer'
import { GroupSystemSkillsAssignment as GroupSystemSkillsAssignmentStore } from '@/modules/skill/widgets/groupSystemSkillsAssignmentDrawer'
import '@/modules/skill/types' // CRITICAL: store declaration merging
import '@/modules/settings/types/SettingsSlots' // settings slot types

const SkillsList = lazyWithPreload(() =>
  import('./components/SkillsList').then(m => ({ default: m.SkillsList })),
)

const AdminSkillsPage = lazyWithPreload(() =>
  import('./components/admin/AdminSkillsPage').then(m => ({
    default: m.AdminSkillsPage,
  })),
)

const GroupSystemSkillsWidget = lazyWithPreload(() =>
  import('./widgets/GroupSystemSkillsWidget').then(m => ({
    default: m.GroupSystemSkillsWidget,
  })),
)

const GroupSystemSkillsAssignmentDrawer = lazyWithPreload(() =>
  import('./widgets/GroupSystemSkillsAssignmentDrawer').then(m => ({
    default: m.GroupSystemSkillsAssignmentDrawer,
  })),
)

export default createModule({
  metadata: {
    name: 'skill',
    version: '1.0.0',
    description: 'Agent Skills — installable knowledge bundles',
  },
  dependencies: ['router'],
  stores: [
    // BOOT-EAGER (always-mounted overlay) — must stay registered.
    { name: 'GroupSystemSkillsAssignment', store: useGroupSystemSkillsAssignmentStore },
  ],
  components: [
    {
      id: 'group-system-skills-assignment-drawer',
      component: GroupSystemSkillsAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => GroupSystemSkillsAssignmentStore.isOpen),
      order: 100,
    },
  ],
  routes: [
    {
      // User Skills now live under Settings (like Workflows / MCP Servers)
      // rather than a top-level sidebar nav item.
      path: '/settings/skills',
      element: SkillsList,
      requiresAuth: true,
      permission: Permissions.SkillsRead,
      layout: SettingsLayoutDef,
    },
    {
      path: '/settings/skills-admin',
      element: AdminSkillsPage,
      requiresAuth: true,
      permission: Permissions.SkillsManageSystem,
      layout: SettingsLayoutDef,
    },
  ],
  slots: {
    settingsUserPages: [
      {
        id: 'skills',
        icon: <BookOpen />,
        label: 'Skills',
        path: 'skills',
        order: 24,
        permission: Permissions.SkillsRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'skills-admin',
        icon: <BookOpen />,
        label: 'System Skills',
        path: 'skills-admin',
        order: 27,
        permission: Permissions.SkillsManageSystem,
      },
    ],
    userGroup: [
      {
        order: 30,
        component: GroupSystemSkillsWidget,
        // Widget loads group system-skills (skills::assign_to_groups).
        permission: Permissions.SkillsAssignToGroups,
      },
    ],
  },
})
