import { createModule } from '@/core'
import { CloudServerOutlined } from '@ant-design/icons'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import {
  useLlmProviderStore,
  useLlmModelDownloadStore,
  useLlmProviderDrawerStore,
  useAddLocalLlmModelUploadDrawerStore,
  useAddLocalLlmModelDownloadDrawerStore,
  useEditLlmModelDrawerStore,
  useAddRemoteLlmModelDrawerStore,
  useViewDownloadDrawerStore,
  useUploadStore,
} from './stores'
import { DownloadIndicatorWidget } from './components/widgets/DownloadIndicatorWidget'
import './types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const LlmProviderSettings = lazyWithPreload(() => import('./components/LlmProviderSettings').then(m => ({ default: m.LlmProviderSettings })))

export default createModule({
  metadata: {
    name: 'llm-provider',
    version: '1.0.0',
    description: 'LLM provider management',
  },
  routes: [
    {
      path: '/settings/llm-providers/:providerId?',
      element: LlmProviderSettings,
      requiresAuth: true,
      layout: SettingsLayout,
    },
  ],
  stores: [
    {
      name: 'LlmProvider',
      store: useLlmProviderStore,
    },
    {
      name: 'LlmModelDownload',
      store: useLlmModelDownloadStore,
    },
    {
      name: 'LlmProviderDrawer',
      store: useLlmProviderDrawerStore,
    },
    {
      name: 'AddLocalLlmModelUploadDrawer',
      store: useAddLocalLlmModelUploadDrawerStore,
    },
    {
      name: 'AddLocalLlmModelDownloadDrawer',
      store: useAddLocalLlmModelDownloadDrawerStore,
    },
    {
      name: 'EditLlmModelDrawer',
      store: useEditLlmModelDrawerStore,
    },
    {
      name: 'AddRemoteLlmModelDrawer',
      store: useAddRemoteLlmModelDrawerStore,
    },
    {
      name: 'ViewDownloadDrawer',
      store: useViewDownloadDrawerStore,
    },
    {
      name: 'LlmModelUpload',
      store: useUploadStore,
    },
  ],
  sidebar: {
    widgets: [
      {
        id: 'download-indicator',
        slot: 'bottom',
        component: <DownloadIndicatorWidget />,
        order: 10,
      },
    ],
  },
  settings: [
    {
      id: 'llm-providers',
      icon: <CloudServerOutlined />,
      label: 'LLM Providers',
      path: 'llm-providers',
      section: 'admin',
      order: 21,
    },
  ],
  initialize: () => {
    console.log('LLM Provider module initialized')
  },
  cleanup: () => {
    console.log('LLM Provider module cleanup')
  },
})
