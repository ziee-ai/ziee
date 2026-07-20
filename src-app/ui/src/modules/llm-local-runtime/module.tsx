import { Server } from 'lucide-react'
import { Permissions } from '@/api-client/permissions'
import { createModule } from '@ziee/framework'
import { SettingsLayoutDef } from '@/modules/settings/SettingsLayout'
import { lazyWithPreload } from '@/utils/lazyWithPreload'
import {
  useRuntimeConfigStore,
  useRuntimeDeleteConfirmStore,
  useRuntimeDownloadDrawerStore,
  useRuntimeDownloadProgressStore,
  useRuntimeModelUsageStore,
  useRuntimeUpdateStore,
  useRuntimeVersionStore,
} from './stores'
import './types' // Register event types

// The Local Runtimes page stacks three independently-gated sections:
//   - version cards          → llm_local_runtime::versions_read (RuntimeVersionRead)
//   - per-instance status/logs inside the version model blocks → LocalRuntimeRead
//   - the singleton runtime-config card → llm_local_runtime::settings_read
// Anyone who can see ANY of those should reach the page (and its menu entry);
// per-section <Can> gates inside the page still hide each card individually.
// Gating the route on LocalRuntimeRead alone both (a) let in principals with
// only instance-status access who then saw an empty page, and (b) LOCKED OUT
// version-only/settings-only readers whose content is the page's main purpose.
// Mirrors the anyOf pattern used by code-sandbox.
const LOCAL_RUNTIME_READ_PERM = {
  anyOf: [
    Permissions.LocalRuntimeRead,
    Permissions.RuntimeVersionRead,
    Permissions.RuntimeSettingsRead,
  ],
}

const RuntimeVersionSettings = lazyWithPreload(() =>
  import('./components/RuntimeVersionSettings').then(m => ({
    default: m.RuntimeVersionSettings,
  })),
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
      permission: LOCAL_RUNTIME_READ_PERM,
      layout: SettingsLayoutDef,
    },
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
    },
    {
      name: 'RuntimeDownloadProgress',
      store: useRuntimeDownloadProgressStore,
    },
  ],

  slots: {
    settingsAdminPages: [
      {
        id: 'llm-runtime',
        icon: <Server />,
        label: 'Local Runtimes',
        // SettingsPage prepends /settings/ to the slot key, so this MUST be
        // a relative segment. The previous absolute path produced
        // /settings//settings/llm-runtime — the URL regex on line 81 of
        // SettingsPage.tsx missed the double slash and bounced users to
        // the first available page. Every other settings module (llm-providers,
        // sandbox, hardware, etc.) uses a relative path here.
        path: 'llm-runtime',
        order: 52, // After LLM Providers (51), before LLM Repositories (53)
        permission: LOCAL_RUNTIME_READ_PERM,
      },
    ],
  },
})
