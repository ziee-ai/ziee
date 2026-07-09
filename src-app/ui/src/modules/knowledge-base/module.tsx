import { Library } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import {
  useKnowledgeBaseComposerStore,
  useKnowledgeBaseDetailStore,
  useKnowledgeBasesStore,
} from '@/modules/knowledge-base/stores'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/knowledge-base/types' // store-merge declaration
// Side-effect imports — register the chat composer/tool-result integration and
// the project "Knowledge bases" knowledge kind even when the respective
// auto-discovery globs don't reach this module first.
import '@/modules/knowledge-base/chat-extension/extension'
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
  dependencies: ['router'],
  stores: [
    { name: 'KnowledgeBases', store: useKnowledgeBasesStore },
    { name: 'KnowledgeBaseDetail', store: useKnowledgeBaseDetailStore },
    { name: 'KnowledgeBaseComposer', store: useKnowledgeBaseComposerStore },
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
