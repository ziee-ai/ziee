import { ApiClient } from '@/api-client'
import {
  type CodeSandboxResourceLimits,
  Permissions,
  type UpdateCodeSandboxResourceLimits,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Runtime-configurable resource caps for the code sandbox (singleton row via
 * `/api/code-sandbox/resource-limits`). Read on first mount, PATCH the diff on
 * save. The server invalidates its in-process cache on a successful PUT so the
 * next `execute_command` picks up the new caps — no restart.
 */
export const SandboxResourceLimits = defineStore('SandboxResourceLimits', {
  immer: true,
  state: {
    limits: null as CodeSandboxResourceLimits | null,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => ({
    loadLimits: async () => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const res = await ApiClient.CodeSandbox.getResourceLimits(undefined)
        set(s => {
          s.limits = res
          s.loading = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to load resource limits'
          s.loading = false
        })
      }
    },
    saveLimits: async (patch: UpdateCodeSandboxResourceLimits) => {
      set(s => {
        s.saving = true
        s.error = null
      })
      try {
        const res = await ApiClient.CodeSandbox.updateResourceLimits(patch)
        set(s => {
          s.limits = res
          s.saving = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to save resource limits'
          s.saving = false
        })
        throw e
      }
    },
  }),
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

export const useSandboxResourceLimitsStore = SandboxResourceLimits.store
