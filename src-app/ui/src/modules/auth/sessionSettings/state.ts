import type { SessionSettings as SessionSettingsRow } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const sessionSettingsState = {
  settings: null as SessionSettingsRow | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type SessionSettingsState = typeof sessionSettingsState
export type SessionSettingsSet = StoreSet<SessionSettingsState>
export type SessionSettingsGet = () => SessionSettingsState
