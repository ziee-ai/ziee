import { BookOpen } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { Stores } from '@/core/stores'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import {
  useConversationSkillsStore,
  useSkillConversationDrawerStore,
  useSkillDrawerStore,
  useSkillStore,
  useSystemSkillStore,
} from '@/modules/skill/stores'
import { useGroupSystemSkillsWidgetStore } from '@/modules/skill/widgets/GroupSystemSkillsWidget.store'
import { useGroupSystemSkillsAssignmentStore } from '@/modules/skill/widgets/GroupSystemSkillsAssignmentDrawer.store'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
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
    { name: 'Skill', store: useSkillStore },
    { name: 'ConversationSkills', store: useConversationSkillsStore },
    { name: 'SystemSkill', store: useSystemSkillStore },
    { name: 'SkillDrawer', store: useSkillDrawerStore },
    {
      name: 'SkillConversationDrawer',
      store: useSkillConversationDrawerStore,
    },
    {
      name: 'GroupSystemSkillsWidget',
      store: useGroupSystemSkillsWidgetStore,
    },
    {
      name: 'GroupSystemSkillsAssignment',
      store: useGroupSystemSkillsAssignmentStore,
    },
  ],
  components: [
    {
      id: 'group-system-skills-assignment-drawer',
      component: GroupSystemSkillsAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.GroupSystemSkillsAssignment.isOpen),
      order: 100,
    },
  ],
  routes: [
    {
      path: '/skills',
      element: SkillsList,
      requiresAuth: true,
      permission: Permissions.SkillsRead,
      layout: AppLayoutDef,
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
    sidebarNavigation: [
      {
        id: 'skills',
        icon: <BookOpen />,
        label: 'Skills',
        path: '/skills',
        order: 80,
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
      },
    ],
  },
})
