import { ApiClient } from '@/api-client'
import {
  type JsToolSettings as JsToolSettingsRow,
  Permissions,
  type UpdateJsToolSettings,
} from '@/api-client/types'
import { hasPermissionNow } from '@/core/permissions'
import { defineStore } from '@ziee/framework/store-kit'

/**
 * Runtime-configurable limits for the built-in `run_js` tool (singleton row via
 * `/api/js-tool/settings`). Read on first mount, PATCH the diff on save. The
 * server invalidates its in-process cache + live-resizes the global admission
 * semaphore on a successful PUT so the next run_js invocation picks up the new
 * caps — no restart. Mirrors `SandboxResourceLimits`.
 */
export const JsToolSettings = defineStore('JsToolSettings', {
  immer: true,
  state: {
    settings: null as JsToolSettingsRow | null,
    loading: false,
    saving: false,
    error: null as string | null,
  },
  actions: set => ({
    loadSettings: async () => {
      set(s => {
        s.loading = true
        s.error = null
      })
      try {
        const res = await ApiClient.JsTool.getSettings(undefined)
        set(s => {
          s.settings = res
          s.loading = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to load run_js limits'
          s.loading = false
        })
      }
    },
    saveSettings: async (patch: UpdateJsToolSettings) => {
      set(s => {
        s.saving = true
        s.error = null
      })
      try {
        const res = await ApiClient.JsTool.updateSettings(patch)
        set(s => {
          s.settings = res
          s.saving = false
        })
      } catch (e: any) {
        set(s => {
          s.error = e?.message ?? 'Failed to save run_js limits'
          s.saving = false
        })
        throw e
      }
    },
  }),
  init: ({ on, actions }) => {
    // Singleton row. Refetch on a remote change or SSE reconnect. Self-gate the
    // refetch (no-403 reconnect rule): sync:reconnect fires for every store
    // regardless of audience, so a user without settings-read must not refetch.
    // The perm MUST equal the GET's read-perm.
    const reload = () => {
      if (!hasPermissionNow(Permissions.JsToolSettingsRead)) return
      void actions.loadSettings()
    }
    on('sync:js_tool_settings', reload)
    on('sync:reconnect', reload)
    void actions.loadSettings()
  },
})

export const useJsToolSettingsStore = JsToolSettings.store
