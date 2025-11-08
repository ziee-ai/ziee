import { Page } from '@playwright/test'

export type ThemePreference = 'light' | 'dark' | 'system'

/**
 * Change the app theme by setting the localStorage value
 * This mimics how the config-client store persists theme preference
 */
export async function setTheme(page: Page, theme: ThemePreference): Promise<void> {
  await page.evaluate((themeValue) => {
    const storageKey = 'config-client-storage'
    const storageData = {
      state: {
        themePreference: themeValue,
      },
      version: 0,
    }
    localStorage.setItem(storageKey, JSON.stringify(storageData))
  }, theme)

  // Reload to apply theme change
  await page.reload({ waitUntil: 'domcontentloaded' })
}

/**
 * Get the current theme preference from localStorage
 */
export async function getTheme(page: Page): Promise<ThemePreference> {
  return await page.evaluate(() => {
    const storageKey = 'config-client-storage'
    const data = localStorage.getItem(storageKey)
    if (data) {
      const parsed = JSON.parse(data)
      return parsed.state?.themePreference || 'system'
    }
    return 'system'
  })
}

/**
 * Check if dark mode is currently active on the page
 */
export async function isDarkMode(page: Page): Promise<boolean> {
  return await page.evaluate(() => {
    return document.documentElement.classList.contains('dark')
  })
}
