import type { JsToolSettings as JsToolSettingsRow } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const jsToolSettingsState = {
  settings: null as JsToolSettingsRow | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type JsToolSettingsState = typeof jsToolSettingsState
export type JsToolSettingsSet = StoreSet<JsToolSettingsState>
export type JsToolSettingsGet = () => JsToolSettingsState
