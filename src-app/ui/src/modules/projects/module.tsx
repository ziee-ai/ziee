import { createModule } from '@/core'
import { FolderOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { Permissions } from '@/api-client/types'

import {
  useProjectsStore,
  useProjectDetailStore,
  useProjectDrawerStore,
} from '@/modules/projects/stores'
import { ConversationProjectChip } from '@/modules/projects/components/ConversationProjectChip'
import '@/modules/projects/types' // store-merge declaration
import '@/modules/projects/events' // event-bus type merge
import '@/modules/chat/types' // chatConversationHeaderTrailing slot decl

const ProjectsListPage = lazyWithPreload(() =>
  import('./pages/ProjectsListPage').then(m => ({
    default: m.ProjectsListPage,
  })),
)
const ProjectDetailPage = lazyWithPreload(() =>
  import('./pages/ProjectDetailPage').then(m => ({
    default: m.ProjectDetailPage,
  })),
)

export default createModule({
  metadata: {
    name: 'projects',
    version: '1.0.0',
    description:
      'Chat Projects: group conversations under shared instructions, knowledge files, and defaults.',
  },
  dependencies: ['router'],
  stores: [
    { name: 'Projects', store: useProjectsStore },
    { name: 'ProjectDetail', store: useProjectDetailStore },
    { name: 'ProjectDrawer', store: useProjectDrawerStore },
  ],
  routes: [
    {
      path: '/projects',
      element: ProjectsListPage,
      requiresAuth: true,
      permission: Permissions.ProjectsRead,
      layout: AppLayoutDef,
    },
    {
      path: '/projects/:projectId',
      element: ProjectDetailPage,
      requiresAuth: true,
      permission: Permissions.ProjectsRead,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarNavigation: [
      {
        id: 'projects',
        icon: <FolderOutlined />,
        label: 'Projects',
        path: '/projects',
        order: 20,
        permission: Permissions.ProjectsRead,
      },
    ],
    // Decorate the chat conversation header (next to TitleEditor)
    // with the "In project: P · N files" chip. The chip itself
    // gates on conversation.project_id being non-null, so it renders
    // nothing for unfiled chats. Registering via slot here decouples
    // the chat module from a direct import of this component
    // (audit N11).
    chatConversationHeaderTrailing: [
      {
        id: 'project-chip',
        component: ConversationProjectChip,
        order: 10,
      },
    ],
  },
  initialize: () => {
    // No imperative init — stores self-bootstrap on first access via __init__.
  },
})
