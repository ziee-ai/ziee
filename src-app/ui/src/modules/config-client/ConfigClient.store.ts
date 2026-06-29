import { create } from 'zustand'
import {
  createJSONStorage,
  persist,
  subscribeWithSelector,
} from 'zustand/middleware'
import type { StoreProxy } from '@/core/stores'

// Guarded persistence storage. Accessing `localStorage` (or writing to it)
// throws in locked-down contexts — private-mode quota, disabled storage, or a
// sandboxed iframe where even reading the property raises SecurityError.
// Without this guard, zustand's default `localStorage`-backed persist would
// throw during store creation and take the whole app down. We probe once and
// fall back to an in-memory store so the app still runs; the preference simply
// won't survive a reload in that environment.
const safeStorage = createJSONStorage(() => {
  try {
    const probe = '__ziee_ls_probe__'
    window.localStorage.setItem(probe, probe)
    window.localStorage.removeItem(probe)
    return window.localStorage
  } catch {
    const mem = new Map<string, string>()
    return {
      getItem: (name: string) => mem.get(name) ?? null,
      setItem: (name: string, value: string) => {
        mem.set(name, value)
      },
      removeItem: (name: string) => {
        mem.delete(name)
      },
    }
  }
})
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
        storage: safeStorage,
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
