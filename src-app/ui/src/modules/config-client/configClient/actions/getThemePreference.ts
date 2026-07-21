import type { ConfigClientGet } from '../state'
import type { ThemePreference } from '../state'

export default (_set: never, get: ConfigClientGet) =>
  async (): Promise<ThemePreference> => get().themePreference
