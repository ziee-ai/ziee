import { createModule } from '@/core'
import { Stores } from '@/core/stores'
import { CloudServerOutlined } from '@ant-design/icons'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
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
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

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
  dependencies: ['router'],
  routes: [
    {
      path: '/settings/llm-providers/:providerId?',
      element: LlmProviderSettings,
      requiresAuth: true,
      layout: SettingsLayoutDef,
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
      name: 'ProviderGroupAssignmentCard',
      store: useProviderGroupCardStore,
    },
  ],
  components: [
    {
      id: 'group-llm-providers-assignment-drawer',
      component: GroupLlmProvidersAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.GroupLlmProvidersAssignment.isOpen),
      order: 100,
    },
    {
      id: 'llm-provider-groups-assignment-drawer',
      component: LlmProviderGroupsAssignmentDrawer,
      shouldMount: () =>
        useDelayedFalse(() => Stores.LlmProviderGroupsAssignment.isOpen),
      order: 101,
    },
  ],
  slots: {
    sidebarBottom: [
      {
        id: 'download-indicator',
        component: DownloadIndicatorWidget,
        order: 10,
      },
    ],
    settingsAdminPages: [
      {
        id: 'llm-providers',
        icon: <CloudServerOutlined />,
        label: 'LLM Providers',
        path: 'llm-providers',
        order: 21,
      },
    ],
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
