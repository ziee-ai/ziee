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
} from '@/modules/llm-provider/stores'
import { useProviderGroupCardStore } from '@/modules/llm-provider/components/ProviderGroupAssignmentCard.store'
import { useLlmProviderGroupWidgetStore } from '@/modules/llm-provider/widgets/LLMProviderGroupWidget.store'
import { useLlmProviderDrawerStore } from '@/modules/llm-provider/components/LlmProviderDrawer.store'
import { useGroupLlmProvidersAssignmentStore } from '@/modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer.store'
import { useLlmProviderGroupsAssignmentStore } from '@/modules/llm-provider/components/LlmProviderGroupsAssignmentDrawer.store'
import { DownloadIndicatorWidget } from '@/modules/llm-provider/components/widgets/DownloadIndicatorWidget'
import '@/modules/llm-provider/types'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import '@/modules/settings/types/SettingsSlots' // Register settings slot types

const LlmProviderSettings = lazyWithPreload(() =>
  import('./components/LlmProviderSettings').then(m => ({
    default: m.LlmProviderSettings,
  })),
)
const GroupLlmProvidersAssignmentDrawer = lazyWithPreload(() =>
  import('./components/GroupLlmProvidersAssignmentDrawer').then(m => ({
    default: m.GroupLlmProvidersAssignmentDrawer,
  })),
)
const LlmProviderGroupsAssignmentDrawer = lazyWithPreload(() =>
  import('./components/LlmProviderGroupsAssignmentDrawer').then(m => ({
    default: m.LlmProviderGroupsAssignmentDrawer,
  })),
)
const LLMProviderGroupWidget = lazyWithPreload(() =>
  import('./widgets/LLMProviderGroupWidget').then(m => ({
    default: m.LLMProviderGroupWidget,
  })),
)

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
