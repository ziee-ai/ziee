import { Mic } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types
import './types' // CRITICAL: enable store type declaration merging

const VOICE_ADMIN_READ_PERM = { anyOf: [Permissions.VoiceAdminRead] }

const VoiceSettingsPage = lazyWithPreload(() =>
  import('./components/VoiceSettingsPage').then(m => ({
    default: m.VoiceSettingsPage,
  })),
)

export default createModule({
  metadata: {
    name: 'voice',
    version: '1.0.0',
    description: 'Voice dictation: whisper runtime + model + settings admin',
  },
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/voice',
      element: VoiceSettingsPage,
      requiresAuth: true,
      permission: VOICE_ADMIN_READ_PERM,
      layout: SettingsLayoutDef,
    },
  ],
  stores: [
  ],
  slots: {
    settingsAdminPages: [
      {
        id: 'voice',
        icon: <Mic />,
        label: 'Voice Dictation',
        path: 'voice',
        order: 62, // After File RAG (61), before Summarization (65).
        permission: VOICE_ADMIN_READ_PERM,
      },
    ],
  },
})
