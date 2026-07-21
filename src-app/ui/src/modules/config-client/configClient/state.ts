import {
  type AccentPreset,
  DEFAULT_ACCENT,
} from '@/components/ThemeProvider/accentPresets'
import type { StoreSet } from '@ziee/framework/store-kit'

export type ThemePreference = 'light' | 'dark' | 'system'

export const configClientState = {
  themePreference: 'system' as ThemePreference,
  /** User-selected brand accent (Settings → Appearance). Drives --primary/--ring. */
  accentPreset: DEFAULT_ACCENT as AccentPreset,
}

export type ConfigClientState = typeof configClientState
export type ConfigClientSet = StoreSet<ConfigClientState>
export type ConfigClientGet = () => ConfigClientState
