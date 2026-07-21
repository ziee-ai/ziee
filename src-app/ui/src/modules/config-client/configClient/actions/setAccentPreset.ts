import {
  ACCENT_PRESETS,
  DEFAULT_ACCENT,
} from '@/components/ThemeProvider/accentPresets'
import type { ConfigClientSet } from '../state'
import type { AccentPreset } from '@/components/ThemeProvider/accentPresets'

// Guard against a stale persisted accent id that no longer exists in code.
const normalizeAccent = (a: AccentPreset): AccentPreset =>
  a in ACCENT_PRESETS ? a : DEFAULT_ACCENT

export default (set: ConfigClientSet) =>
  async (preset: AccentPreset) => {
    set({ accentPreset: normalizeAccent(preset) })
  }
