import { createModule } from '@/core'
import { CloudServerOutlined } from '@ant-design/icons'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import {
  useRuntimeVersionStore,
  useRuntimeUpdateStore,
  useRuntimeDownloadDrawerStore,
  useRuntimeDeleteConfirmStore,
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
    }
  ],

  slots: {
    settingsAdminPages: [
      {
        id: 'llm-runtime',
        icon: <CloudServerOutlined />,
        label: 'Local Runtimes',
        path: '/settings/llm-runtime',
        order: 52, // After LLM Providers (51), before LLM Repositories (53)
        permission: 'llm_local_runtime::read',
      }
    ]
  }
})
