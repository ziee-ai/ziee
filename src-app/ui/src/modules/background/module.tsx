import { Bot } from 'lucide-react'

import { createModule } from '@ziee/framework'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

import {
  BACKGROUND_USE_PERMISSION,
  useBackgroundRunsStore,
} from './stores/BackgroundRuns.store'
import '@/modules/background/types' // register Stores.BackgroundRuns (declaration merge)

const BackgroundTasksPage = lazyWithPreload(() =>
  import('./pages/BackgroundTasksPage').then(m => ({
    default: m.BackgroundTasksPage,
  })),
)

export default createModule({
  metadata: {
    name: 'background',
    version: '1.0.0',
    description: 'Background sub-agent runs — view, cancel, and steer detached tasks',
  },
  dependencies: ['router'],
  routes: [
    {
      // Top-level nav destination → render inside the app shell (left sidebar +
      // header bar) so the user keeps the sidebar to hop back to chat, matching
      // every other top-level page (chat/scheduled-tasks/knowledge-base).
      path: '/background-tasks',
      element: BackgroundTasksPage,
      requiresAuth: true,
      // Same read perm the /api/background/runs endpoint enforces (`background::use`).
      // NOTE: not yet in the generated `Permissions` enum — see BACKGROUND_USE_PERMISSION.
      permission: BACKGROUND_USE_PERMISSION,
      layout: AppLayoutDef,
    },
  ],
  stores: [{ name: 'BackgroundRuns', store: useBackgroundRunsStore }],
  slots: {
    sidebarNavigation: [
      {
        // Discoverable entry to the live background-run dashboard. Sits between
        // "Scheduled Tasks" (order 22) and "Background results" (order 24) so all
        // background/agent work is grouped together in the nav.
        id: 'background-tasks',
        icon: <Bot />,
        label: 'Background tasks',
        path: '/background-tasks',
        order: 23,
        // Gate: SAME read perm as the route + the runs data (`background::use`).
        // A user without the grant never sees the entry (and the store self-gates
        // its fetch → no 403).
        permission: BACKGROUND_USE_PERMISSION,
      },
    ],
  },
})
