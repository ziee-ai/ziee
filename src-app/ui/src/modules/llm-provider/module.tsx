import { Server } from 'lucide-react'
import { Permissions } from '@/api-client/types'
import { createModule } from '@ziee/framework'
import { Stores } from '@ziee/framework/stores'
import { useGroupLlmProvidersAssignmentStore } from '@/modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer.store'
import { useLlmProviderDrawerStore } from '@/modules/llm-provider/components/LlmProviderDrawer.store'
import { useProviderGroupCardStore } from '@/modules/llm-provider/components/ProviderGroupAssignmentCard.store'
import { DownloadIndicatorWidget } from '@/modules/llm-provider/components/widgets/DownloadIndicatorWidget'
import {
  useAddLocalLlmModelDownloadDrawerStore,
  useAddLocalLlmModelUploadDrawerStore,
  useAddRemoteLlmModelDrawerStore,
  useEditLlmModelDrawerStore,
  useLlmModelDownloadStore,
  useLlmProviderStore,
  useUploadStore,
  useViewDownloadDrawerStore,
} from '@/modules/llm-provider/stores'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import '@/modules/llm-provider/types'
import { useDelayedFalse } from '@/hooks/useDelayedFalse'
import { usePermission } from '@/core/permissions'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
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
const LLMProviderGroupWidget = lazyWithPreload(() =>
  import('./widgets/LLMProviderGroupWidget').then(m => ({
    default: m.LLMProviderGroupWidget,
  })),
)
const LlmModelDownloadNotifications = lazyWithPreload(() =>
  import('./components/LlmModelDownloadNotifications').then(m => ({
    default: m.LlmModelDownloadNotifications,
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
      permission: Permissions.LlmProvidersRead,
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
      // Globally-mounted listener for the
      // `llm_model.download_{completed,failed}` events emitted from
      // the LlmModelDownload store's SSE handler. Surfaces toasts so
      // the user sees completion / failure regardless of which page
      // they're on at the time (the hub model card alone only updates
      // for users still on the Hub page). Renders null; safe to mount
      // always.
      id: 'llm-model-download-notifications',
      component: LlmModelDownloadNotifications,
      // Gate: download activity is admin-managed (`llm_models::downloads_read`).
      // A non-admin (and a logged-out visitor) can't see downloads, so don't
      // load this listener's chunk for them — matches the DownloadIndicatorWidget
      // slot's permission gate.
      shouldMount: () => usePermission(Permissions.LlmModelsDownloadsRead),
      order: 102,
    },
  ],
  slots: {
    sidebarBottom: [
      {
        id: 'download-indicator',
        component: DownloadIndicatorWidget,
        order: 10,
        // Gate: exposes in-flight model-download details (model names,
        // repo paths) + Retry/Clear controls. Downloads are admin-managed
        // (`llm_models::downloads_*`). Without this the widget only
        // self-hid on an empty store; gate it on downloads_read so a
        // non-admin never sees download activity nor a 403 fetch.
        permission: Permissions.LlmModelsDownloadsRead,
      },
    ],
    settingsAdminPages: [
      {
        id: 'llm-providers',
        icon: <Server />,
        label: 'LLM Providers',
        path: 'llm-providers',
        order: 21,
        permission: Permissions.LlmProvidersRead,
      },
    ],
    userGroup: [
      {
        order: 10,
        component: LLMProviderGroupWidget,
        // Widget loads GET /api/groups/{id}/providers (llm_providers::read).
        permission: Permissions.LlmProvidersRead,
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
