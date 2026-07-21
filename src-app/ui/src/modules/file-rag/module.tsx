import { FileSearch } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useFileRagAdminStore } from './stores/fileRagAdmin'
import './types'

const FileRagAdminPage = lazyWithPreload(() =>
  import('./pages/FileRagAdminPage').then(m => ({ default: m.FileRagAdminPage })),
)

export default createModule({
  metadata: {
    name: 'file_rag',
    version: '1.0.0',
    description: 'Document RAG: semantic + full-text search over project/conversation files.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/file-rag-admin',
      element: FileRagAdminPage,
      requiresAuth: true,
      permission: Permissions.FileRagAdminRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [{ name: 'FileRagAdmin', store: useFileRagAdminStore }],
  slots: {
    settingsAdminPages: [
      {
        id: 'file-rag-admin',
        icon: <FileSearch />,
        label: 'Document RAG',
        // Single-segment path so SettingsPage's section regex highlights it.
        path: 'file-rag-admin',
        order: 61, // Right after Memory admin (order 60).
        permission: Permissions.FileRagAdminRead,
      },
    ],
  },
})
