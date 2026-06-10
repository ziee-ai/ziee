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
  await page.waitForLoadState('load')
}

export async function waitForSettingsPageLoad(page: Page, expectedText: string) {
  // Wait for the heading specifically to avoid strict mode violations
  await page.getByRole('heading', { name: expectedText }).waitFor({ timeout: 30000 })
}
