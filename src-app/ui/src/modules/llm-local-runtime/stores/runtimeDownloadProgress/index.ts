import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { Permissions } from '@/api-client/permissions'
import { hasPermissionNow } from '@/core/permissions'
import { runtimeDownloadProgressState, type RuntimeDownloadProgressState } from './state'
import type { Actions } from './actions.gen'

const RuntimeDownloadProgressDef = defineStore<RuntimeDownloadProgressState, Actions>(
  'RuntimeDownloadProgress',
  {
    immer: true,
    state: runtimeDownloadProgressState,
    actions: import.meta.glob('./actions/*.ts'),
    init: ({ actions }) => {
      if (!hasPermissionNow(Permissions.RuntimeVersionRead)) return
      void actions.loadActive()
    },
  },
)

// `registerLazyStore` registers the raw definition in the `Stores` global.
// We re-export the raw definition as `RuntimeDownloadProgress` so gallery code
// that does `RuntimeDownloadProgress.store.setState(...)` continues to work.
registerLazyStore(RuntimeDownloadProgressDef)
export { RuntimeDownloadProgressDef as RuntimeDownloadProgress }
export const useRuntimeDownloadProgressStore = RuntimeDownloadProgressDef.store
