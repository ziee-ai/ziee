import { defineStore, registerLazyStore } from '@ziee/framework/store-kit'
import { sandboxResourceLimitsState, type SandboxResourceLimitsState } from './state'
import type { Actions } from './actions.gen'
import { hasPermissionNow } from '@/core/permissions'
import { Permissions } from '@/api-client/permissions'

export const SandboxResourceLimitsDef = defineStore<SandboxResourceLimitsState, Actions>('SandboxResourceLimits', {
  immer: true,
  state: sandboxResourceLimitsState,
  actions: import.meta.glob('./actions/*.ts'),
  init: ({ on, actions }) => {
    // Singleton row. Refetch on a remote change or SSE reconnect. Self-gate the
    // refetch (no-403 reconnect rule): sync:reconnect fires for every store
    // regardless of audience, so a user without resource-limits read must not
    // refetch. The perm MUST equal the GET's read-perm.
    const reload = () => {
      if (!hasPermissionNow(Permissions.CodeSandboxResourceLimitsRead)) return
      void actions.loadLimits()
    }
    on('sync:code_sandbox_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadLimits()
  },
})

export const SandboxResourceLimits = registerLazyStore(SandboxResourceLimitsDef)
export const useSandboxResourceLimitsStore = SandboxResourceLimitsDef.store
