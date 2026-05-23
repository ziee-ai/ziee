import { create } from 'zustand'
import { subscribeWithSelector } from 'zustand/middleware'
import { immer } from 'zustand/middleware/immer'
import { ApiClient } from '@/api-client'
import type {
  CodeSandboxResourceLimits,
  UpdateCodeSandboxResourceLimits,
} from '@/api-client/types'

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
    limits?: () => Promise<void>
  }

  loadLimits: () => Promise<void>
  saveLimits: (patch: UpdateCodeSandboxResourceLimits) => Promise<void>
}

export const useSandboxResourceLimitsStore =
  create<SandboxResourceLimitsStore>()(
    subscribeWithSelector(
      immer((set, _get) => ({
        limits: null,
        loading: false,
        saving: false,
        error: null,

        __init__: {
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
      })),
    ),
  )
