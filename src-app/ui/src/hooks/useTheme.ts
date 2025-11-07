import { createContext, useContext } from 'react'
import { AppThemeConfig } from '@/themes/light'
import type { ThemePreference } from '@/modules/config-client/ConfigClient.store'

export type ThemeName = 'light' | 'dark'

export interface ThemeContextValue {
  currentTheme: AppThemeConfig
  selectedTheme: ThemePreference
  resolvedTheme: ThemeName
  isDarkMode: boolean
  setTheme: (theme: ThemePreference) => void
}

export const ThemeContext = createContext<ThemeContextValue | undefined>(
  undefined,
)

export function useTheme() {
  const context = useContext(ThemeContext)
  if (!context) {
    throw new Error('useTheme must be used within ThemeProvider')
  }
  return context
}
