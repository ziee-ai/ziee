import { createModule } from '@/core'
import { FolderAddOutlined, FolderOutlined } from '@ant-design/icons'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { Permissions } from '@/api-client/types'

import {
  useProjectsStore,
  useProjectDetailStore,
  useProjectDrawerStore,
} from '@/modules/projects/stores'
import { ProjectsNavWidget } from '@/modules/projects/widgets/ProjectsNavWidget'
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
    // Note: SidebarActionItem doesn't currently support a permission
    // field (only SidebarNavItem does). Since this button just routes
    // to /projects (which is permission-gated at the route level), an
    // unauthorized user clicking it will be bounced by the router's
    // permission guard. A future improvement is to add a permission
    // field to SidebarActionItem to hide it pre-emptively.
    sidebarPrimaryActions: [
      {
        id: 'new-project',
        icon: <FolderAddOutlined />,
        label: 'New Project',
        to: '/projects',
        order: 20,
      },
    ],
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
    // Inline list of recent projects, sits ABOVE the chat module's
    // RecentConversationsWidget (which is at order: 10) so the
    // "Projects" sidebar group is the first thing the user sees.
    sidebarContent: [
      {
        id: 'projects-nav',
        component: ProjectsNavWidget,
        order: 5,
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
