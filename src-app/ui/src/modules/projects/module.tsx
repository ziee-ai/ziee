import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import {
  useProjectDetailStore,
  useProjectDrawerStore,
  useProjectsStore,
} from '@/modules/projects/stores'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/projects/types' // store-merge declaration
import '@/modules/projects/events' // event-bus type merge
// Trigger the auto-discovery glob — sibling modules with a
// `project-extension/extension.tsx` register their knowledge-kind
// contributions side-effectfully at module-import time. Side-effect
// import only — no symbol referenced here.
import '@/modules/projects/extensions'

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
// Project-namespaced chat URL renders chat's existing ConversationPage
// as-is. Route registration is the one place projects/ core reaches
// into chat module — out of scope for the project↔chat inversion
// round (no `routes` slot on the frontend chat-extension framework
// yet). If/when that slot lands, move the lazy-import into
// `projects/chat-extension/extension.tsx`.
const ProjectChatPage = lazyWithPreload(
  () => import('@/modules/chat/pages/ConversationPage'), // chat-extension-boundary-exception
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
    {
      // Project-namespaced chat URL. Project module owns the URL
      // shape; chat doesn't know about projects. Both this URL and
      // the plain `/chat/:conversationId` are valid for a project-
      // bound conversation (no redirect between them); links FROM
      // project surfaces resolve through the `conversationHref`
      // extension hook so they use this namespaced form by default.
      path: '/projects/:projectId/chat/:conversationId',
      element: ProjectChatPage,
      requiresAuth: true,
      permission: Permissions.ProjectsRead,
      layout: AppLayoutDef,
    },
  ],
  // Projects intentionally contribute NO left-sidebar entries (removed the
  // sidebarNavigation link + the ProjectsNavWidget). The routes stay valid so
  // project-scoped chat URLs + in-chat project affordances keep working.
  initialize: () => {
    // No imperative init — stores self-bootstrap on first access via __init__.
  },
})
