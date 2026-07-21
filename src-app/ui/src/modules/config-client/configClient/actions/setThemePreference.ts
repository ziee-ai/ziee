import type { ConfigClientSet } from '../state'
import type { ThemePreference } from '../state'

export default (set: ConfigClientSet) =>
  async (preference: ThemePreference) => {
    set({ themePreference: preference })
  }
