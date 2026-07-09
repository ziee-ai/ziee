import { Library } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { AppLayoutDef } from '@/modules/layouts/app-layout'
import {
  useKnowledgeBaseDetailStore,
  useKnowledgeBasesStore,
} from '@/modules/knowledge-base/stores'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/knowledge-base/types' // store-merge declaration

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
