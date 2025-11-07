import { createModule } from '@/core'
import { CloudServerOutlined } from '@ant-design/icons'
import SettingsLayout from '@/modules/settings/SettingsLayout'
import {
  useLlmProviderStore,
  useLlmModelDownloadStore,
  useAddLocalLlmModelUploadDrawerStore,
  useAddLocalLlmModelDownloadDrawerStore,
  useEditLlmModelDrawerStore,
  useAddRemoteLlmModelDrawerStore,
  useViewDownloadDrawerStore,
  useUploadStore,
} from './stores'
import { useProviderGroupCardStore } from './components/ProviderGroupAssignmentCard.store'
import { useLlmProviderGroupWidgetStore } from './widgets/LLMProviderGroupWidget.store'
import { useLlmProviderDrawerStore } from './components/LlmProviderDrawer.store'
import { useGroupLlmProvidersAssignmentStore } from './components/GroupLlmProvidersAssignmentDrawer.store'
import { useLlmProviderGroupsAssignmentStore } from './components/LlmProviderGroupsAssignmentDrawer.store'
import { DownloadIndicatorWidget } from './components/widgets/DownloadIndicatorWidget'
import './types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'

const LlmProviderSettings = lazyWithPreload(() => import('./components/LlmProviderSettings').then(m => ({ default: m.LlmProviderSettings })))
const GroupLlmProvidersAssignmentDrawer = lazyWithPreload(() => import('./components/GroupLlmProvidersAssignmentDrawer').then(m => ({ default: m.GroupLlmProvidersAssignmentDrawer })))
const LlmProviderGroupsAssignmentDrawer = lazyWithPreload(() => import('./components/LlmProviderGroupsAssignmentDrawer').then(m => ({ default: m.LlmProviderGroupsAssignmentDrawer })))
const LLMProviderGroupWidget = lazyWithPreload(() => import('./widgets/LLMProviderGroupWidget').then(m => ({ default: m.LLMProviderGroupWidget })))

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
    {
      name: 'GroupLlmProvidersAssignment',
      store: useGroupLlmProvidersAssignmentStore,
    },
    {
      name: 'LlmProviderGroupsAssignment',
      store: useLlmProviderGroupsAssignmentStore,
    },
    {
      name: 'LlmProviderGroupWidget',
      store: useLlmProviderGroupWidgetStore,
    },
    {
      name: 'ProviderGroupCard',
      store: useProviderGroupCardStore,
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
  globalComponents: [
    {
      id: 'group-llm-providers-assignment-drawer',
      component: GroupLlmProvidersAssignmentDrawer,
    },
    {
      id: 'llm-provider-groups-assignment-drawer',
      component: LlmProviderGroupsAssignmentDrawer,
    },
  ],
  widgets: {
    userGroup: [
      {
        order: 10,
        component: LLMProviderGroupWidget,
      },
    ],
  },
  initialize: () => {
    console.log('LLM Provider module initialized')
  },
  cleanup: () => {
    console.log('LLM Provider module cleanup')
  },
})
