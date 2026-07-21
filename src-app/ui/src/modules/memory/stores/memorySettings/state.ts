import type { StoreSet } from '@ziee/framework/store-kit'
import type { UserMemorySettings } from '@/api-client/types'

export const memorySettingsState = {
  settings: null as UserMemorySettings | null,
  loading: false,
  saving: false,
  error: null as string | null,
}

export type MemorySettingsState = typeof memorySettingsState
export type MemorySettingsSet = StoreSet<MemorySettingsState>
export type MemorySettingsGet = () => MemorySettingsState
