import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import {
  type CodeSandboxResourceLimits,
  type UpdateCodeSandboxResourceLimits,
} from '@/api-client/types'
import { Stores } from '@/core/stores'

/**
 * Runtime-configurable resource caps for the code sandbox (Plan 1 §6).
 *
 * Single singleton row exposed via `/api/code-sandbox/resource-limits`.
 * The page reads on first mount via the `__init__.limits` hook, then PATCHes
 * the diff on save. The server invalidates its in-process cache on a
 * successful PUT, so the next `execute_command` picks up the new caps —
 * no restart needed.
 */
interface SandboxResourceLimitsStore {
  limits: CodeSandboxResourceLimits | null
  loading: boolean
  saving: boolean
  error: string | null

  __init__: {
    __store__?: () => void
    limits?: () => Promise<void>
  }

  __destroy__?: () => void

  loadLimits: () => Promise<void>
  saveLimits: (patch: UpdateCodeSandboxResourceLimits) => Promise<void>
}

export const useSandboxResourceLimitsStore =
  create<SandboxResourceLimitsStore>()(
    subscribeWithSelector(
      immer((set, get) => ({
        limits: null,
        loading: false,
        saving: false,
        error: null,

        __init__: {
          __store__: () => {
            const eventBus = Stores.EventBus
            const GROUP = 'SandboxResourceLimitsStore'
            // Code-sandbox resource-limit settings (singleton). Refetch on a
            // remote change (the event id is nil — it's a singleton row) or on
            // SSE reconnect.
            const reload = () => void get().loadLimits()
            eventBus.on('sync:code_sandbox_settings', reload, GROUP)
            eventBus.on('sync:reconnect', reload, GROUP)
          },
          limits: async () => {
            set(s => {
              s.loading = true
              s.error = null
            })
            try {
              const res =
                await ApiClient.CodeSandbox.getResourceLimits(undefined)
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
        },

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

        __destroy__: () => {
          Stores.EventBus.removeGroupListeners('SandboxResourceLimitsStore')
        },
      })),
    ),
  )
