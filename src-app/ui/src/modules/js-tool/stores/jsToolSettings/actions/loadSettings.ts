import { ApiClient } from '@/api-client'
import type { JsToolSettingsGet, JsToolSettingsSet } from '../state'

export default (set: JsToolSettingsSet, _get: JsToolSettingsGet) =>
  async () => {
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
  }
