import { Library } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/knowledge-base/types' // store-merge declaration
// Side-effect imports — register the chat composer/tool-result integration and
// the project "Knowledge bases" knowledge kind even when the respective
// auto-discovery globs don't reach this module first.
// The chat-extension is auto-discovered lazily by chat's extension glob (loaded
// with the /chat page, not at boot) — importing it here dragged the kb chat
// slots + ChatPaneContext + the File store into the boot payload. The
// project-extension (below) stays eager: it registers the "Knowledge bases"
// project knowledge-kind, which the projects surface needs independent of chat.
import '@/modules/knowledge-base/project-extension/extension'

const KnowledgeBasesListPage = lazyWithPreload(() =>
  import('./pages/KnowledgeBasesListPage').then(m => ({
    default: m.KnowledgeBasesListPage,
  })),
)
const KnowledgeBaseDetailPage = lazyWithPreload(() =>
  import('./pages/KnowledgeBaseDetailPage').then(m => ({
    default: m.KnowledgeBaseDetailPage,
  })),
)

export default createModule({
  metadata: {
    name: 'knowledge-base',
    version: '1.0.0',
    description:
      'Knowledge bases: named, reusable collections the agent retrieves from (RAG at scale).',
  },
  // smart-loading gate (build-lifted into the manifest)
  shouldLoad: (ctx) => ctx.isAuthenticated,
  dependencies: ['router'],
  stores: [
  ],
  routes: [
    {
      path: '/knowledge',
      element: KnowledgeBasesListPage,
      requiresAuth: true,
      permission: Permissions.KnowledgeBaseUse,
      layout: AppLayoutDef,
    },
    {
      path: '/knowledge/:kbId',
      element: KnowledgeBaseDetailPage,
      requiresAuth: true,
      permission: Permissions.KnowledgeBaseUse,
      layout: AppLayoutDef,
    },
  ],
  slots: {
    sidebarNavigation: [
      {
        id: 'knowledge',
        icon: <Library />,
        label: 'Knowledge',
        path: '/knowledge',
        order: 15,
        permission: Permissions.KnowledgeBaseUse,
      },
    ],
  },
  initialize: () => {
    // Stores self-bootstrap on first access.
  },
})
