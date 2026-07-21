import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import {
  runtimeModelUsageState,
  type RuntimeModelUsageState,
} from './state'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import type { Actions } from './actions.gen'

const RuntimeModelUsageDef = defineStore<RuntimeModelUsageState, Actions>(
  'RuntimeModelUsage',
  {
    state: runtimeModelUsageState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ on, get, actions }) => {
      // Re-resolve usage when versions change elsewhere: a download adds a version,
      // a delete removes one, and a default change alters which version unpinned
      // models resolve to.
      const reload = () => {
        // Self-gate so a non-runtime-admin never 403s on a reconnect.
        if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
        for (const engine of get().usage.keys()) void actions.loadUsage(engine)
      }
      on('runtime_version.created', reload)
      on('runtime_version.deleted', reload)
      on('runtime_version.default_changed', reload)
      // Cross-device: RuntimeVersion.store refetches on sync:runtime_version but
      // does NOT re-emit local runtime_version.* events, so subscribe directly.
      on('sync:runtime_version', reload)
      on('sync:reconnect', reload)
    },
  },
)

// The raw Zustand store for gallery setup that needs direct setState.
export const RuntimeModelUsageStore = RuntimeModelUsageDef.store

export const RuntimeModelUsage = registerLazyStore(RuntimeModelUsageDef)
export const useRuntimeModelUsageStore = RuntimeModelUsageDef.store
