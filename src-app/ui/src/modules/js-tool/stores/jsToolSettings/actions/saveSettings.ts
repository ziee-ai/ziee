import { ApiClient } from '@/api-client'
import { type UpdateJsToolSettings } from '@/api-client/types'
import type { JsToolSettingsGet, JsToolSettingsSet } from '../state'

export default (set: JsToolSettingsSet, _get: JsToolSettingsGet) =>
  async (patch: UpdateJsToolSettings) => {
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
  }
