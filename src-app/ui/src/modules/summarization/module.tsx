import { CompressOutlined } from '@ant-design/icons'
import { Permissions } from '@/api-client/types'
import { createModule } from '@/core'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useConversationSummarizationStore } from './stores/ConversationSummarization.store'
import { useSummarizationAdminStore } from './stores/SummarizationAdmin.store'
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
    { name: 'SummarizationAdmin', store: useSummarizationAdminStore },
    {
      name: 'ConversationSummarization',
      store: useConversationSummarizationStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'summarization-admin',
        icon: <CompressOutlined />,
        label: 'Summarization',
        path: 'summarization-admin',
        order: 65,
        permission: Permissions.SummarizationSettingsRead,
      },
    ],
  },
})
