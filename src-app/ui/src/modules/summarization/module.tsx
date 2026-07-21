import { Shrink } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import './types'

const SummarizationAdminPage = lazyWithPreload(() =>
  import('./pages/SummarizationAdminPage').then(m => ({
    default: m.SummarizationAdminPage,
  })),
)

export default createModule({
  metadata: {
    name: 'summarization',
    version: '1.0.0',
    description:
      'Conversation summarization: rolling per-branch context compaction.',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/summarization-admin',
      element: SummarizationAdminPage,
      requiresAuth: true,
      permission: Permissions.SummarizationSettingsRead,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'summarization-admin',
        icon: <Shrink />,
        label: 'Summarization',
        path: 'summarization-admin',
        order: 65,
        permission: Permissions.SummarizationSettingsRead,
      },
    ],
  },
})
