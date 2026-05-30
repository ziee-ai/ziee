import { createModule } from '@/core'
import { CloudServerOutlined } from '@ant-design/icons'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { Permissions } from '@/api-client/types'
import {
  useRuntimeVersionStore,
  useRuntimeUpdateStore,
  useRuntimeDownloadDrawerStore,
  useRuntimeDeleteConfirmStore,
  useRuntimeConfigStore,
  useRuntimeModelUsageStore,
} from './stores'
import './types' // Register event types

const RuntimeVersionSettings = lazyWithPreload(() =>
  import('./components/RuntimeVersionSettings').then(m => ({
    default: m.RuntimeVersionSettings
  }))
)

export default createModule({
  metadata: {
    name: 'llm-local-runtime',
    version: '1.0.0',
    description: 'Local LLM runtime version management',
  },
  routes: [
    {
      path: '/settings/llm-runtime',
      element: RuntimeVersionSettings,
      requiresAuth: true,
      permission: Permissions.LocalRuntimeRead,
      layout: SettingsLayoutDef,
    }
  ],

  stores: [
    {
      name: 'RuntimeVersion',
      store: useRuntimeVersionStore,
    },
    {
      name: 'RuntimeUpdate',
      store: useRuntimeUpdateStore,
    },
    {
      name: 'RuntimeDownloadDrawer',
      store: useRuntimeDownloadDrawerStore,
    },
    {
      name: 'RuntimeDeleteConfirm',
      store: useRuntimeDeleteConfirmStore,
    },
    {
      name: 'RuntimeConfig',
      store: useRuntimeConfigStore,
    },
    {
      name: 'RuntimeModelUsage',
      store: useRuntimeModelUsageStore,
    }
  ],

  slots: {
    settingsAdminPages: [
      {
        id: 'llm-runtime',
        icon: <CloudServerOutlined />,
        label: 'Local Runtimes',
        // SettingsPage prepends /settings/ to the slot key, so this MUST be
        // a relative segment. The previous absolute path produced
        // /settings//settings/llm-runtime — the URL regex on line 81 of
        // SettingsPage.tsx missed the double slash and bounced users to
        // the first available page. Every other settings module (llm-providers,
        // sandbox, hardware, etc.) uses a relative path here.
        path: 'llm-runtime',
        order: 52, // After LLM Providers (51), before LLM Repositories (53)
        permission: Permissions.LocalRuntimeRead,
      }
    ]
  }
})
