export { lightTheme } from './light.ts'
export { darkTheme } from './dark.ts'

import { lightTheme } from './light.ts'
import { darkTheme } from './dark.ts'

export const themes = {
  light: lightTheme,
  dark: darkTheme,
} as const

export type ThemeName = keyof typeof themes
