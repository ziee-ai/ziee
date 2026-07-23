import { Bot } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'

import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import { useAgentAdminSettingsStore } from './stores/AgentAdminSettings.store'
import './events' // Register agent event type declaration merging
import './types' // CRITICAL: enable store type declaration merging

const AgentSettingsPage = lazyWithPreload(() =>
  import('./AgentSettingsPage').then(m => ({
    default: m.AgentSettingsPage,
  })),
)

// Admin-only surface. The read permission gates the menu entry, the route, and
// the page render; the section additionally read-only-locks the form without
// `agent::settings::manage`.
const AGENT_READ_PERM = Permissions.AgentSettingsRead

export default createModule({
  metadata: {
    name: 'agent',
    version: '1.0.0',
    description: 'Deployment-wide agent policy administration',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/agent',
      element: AgentSettingsPage,
      requiresAuth: true,
      permission: AGENT_READ_PERM,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
    {
      name: 'AgentAdminSettings',
      store: useAgentAdminSettingsStore,
    },
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'agent',
        icon: <Bot />,
        label: 'Agent',
        path: 'agent',
        order: 24,
        permission: AGENT_READ_PERM,
      },
    ],
  },
})
