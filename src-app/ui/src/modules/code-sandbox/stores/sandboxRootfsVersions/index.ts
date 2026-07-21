import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { sandboxRootfsVersionsState, type SandboxRootfsVersionsState } from './state'
import type { Actions } from './actions.gen'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'
import { setSubscribeWires } from './actions/subscribeToInstallProgress'

export const SandboxRootfsVersionsDef = defineStore<SandboxRootfsVersionsState, Actions>('SandboxRootfsVersions', {
  immer: true,
  state: sandboxRootfsVersionsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, onCleanup, actions }) => {
    // Wire cross-references for SSE callbacks.
    setSubscribeWires({
      loadStatus: actions.loadStatus,
      self: actions.subscribeToInstallProgress,
    })

    // Eager-load status + open the install-progress SSE on mount. The backend
    // enforces the permission, so always load (a permission snapshot here runs
    // before auth populates and would permanently skip the load).
    void (async () => {
      await actions.loadStatus({ pruneFailed: true })
      void actions.subscribeToInstallProgress()
    })()
    // Cross-device sync: another admin installed/evicted/deleted a version.
    // Self-gated (sync events fire after auth populates, so the snapshot is reliable).
    const reload = () => {
      if (!hasPermissionNow(Permissions.CodeSandboxEnvironmentsRead)) return
      void actions.loadStatus()
    }
    on('sync:code_sandbox_rootfs_version', reload)
    on('sync:reconnect', reload)
    onCleanup(() => {
      actions.cleanupSse()
    })
  },
})

export const SandboxRootfsVersions = registerLazyStore(SandboxRootfsVersionsDef)
export const useSandboxRootfsVersionsStore = SandboxRootfsVersionsDef.store
