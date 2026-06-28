import { create } from 'zustand'
import { persist, subscribeWithSelector } from 'zustand/middleware'
import type { StoreProxy } from '@/core/stores'
import {
  type AccentPreset,
  DEFAULT_ACCENT,
  ACCENT_PRESETS,
} from '@/components/ThemeProvider/accentPresets'

export type ThemePreference = 'light' | 'dark' | 'system'

interface ConfigClientState {
  themePreference: ThemePreference
  /** User-selected brand accent (Settings → Appearance). Drives --primary/--ring. */
  accentPreset: AccentPreset

  // Actions
  setThemePreference: (preference: ThemePreference) => void
  getThemePreference: () => ThemePreference
  setAccentPreset: (preset: AccentPreset) => void
}

// Augment RegisteredStores for IntelliSense
declare module '../../core/stores' {
  interface RegisteredStores {
    ConfigClient: StoreProxy<ConfigClientState>
  }
}

const defaultState = {
  themePreference: 'system' as ThemePreference,
  accentPreset: DEFAULT_ACCENT,
}

// Guard against a stale persisted accent id that no longer exists in code.
const normalizeAccent = (a: AccentPreset): AccentPreset =>
  a in ACCENT_PRESETS ? a : DEFAULT_ACCENT

export const useConfigClientStore = create<ConfigClientState>()(
  subscribeWithSelector(
    persist(
      (set, get): ConfigClientState => ({
        ...defaultState,

        // Actions
        setThemePreference: (preference: ThemePreference) => {
          set({ themePreference: preference })
        },

        getThemePreference: () => {
          return get().themePreference
        },

        setAccentPreset: (preset: AccentPreset) => {
          set({ accentPreset: normalizeAccent(preset) })
        },
      }),
      {
        name: 'config-client-storage',
        partialize: state => ({
          themePreference: state.themePreference,
          accentPreset: state.accentPreset,
        }),
        merge: (persisted, current) => {
          const p = (persisted ?? {}) as Partial<ConfigClientState>
          return {
            ...current,
            ...p,
            accentPreset: normalizeAccent(p.accentPreset ?? current.accentPreset),
          }
        },
      },
    ),
  ),
)
