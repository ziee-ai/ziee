import { ReadOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import {
  useConversationSkillsStore,
  useSkillConversationDrawerStore,
  useSkillDrawerStore,
  useSkillStore,
  useSystemSkillStore,
} from '@/modules/skill/stores'
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
        icon: <ReadOutlined />,
        label: 'Skills',
        path: '/skills',
        order: 80,
        permission: Permissions.SkillsRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'skills-admin',
        icon: <ReadOutlined />,
        label: 'System Skills',
        path: 'skills-admin',
        order: 27,
        permission: Permissions.SkillsManageSystem,
      },
    ],
  },
})
