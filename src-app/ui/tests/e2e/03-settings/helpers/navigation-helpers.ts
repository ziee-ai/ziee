import { Page } from '@playwright/test'

/**
 * Settings-specific navigation helpers
 */

export type SettingsPage = 'general' | 'hardware' | 'llm-providers' | 'llm-repositories'

export async function goToSettingsPage(
  page: Page,
  baseURL: string,
  settingsPath: SettingsPage
) {
  await page.goto(`${baseURL}/settings/${settingsPath}`)
  await page.waitForLoadState('networkidle')
}

export async function waitForSettingsPageLoad(page: Page, expectedText: string) {
  await page.waitForSelector(`text=${expectedText}`, { timeout: 30000 })
}
